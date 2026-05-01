ClipCannon and the Teleological Constellation System
A Unified White Paper on Identity-Aligned Video Understanding, Editing, and Avatar Generation
Author: Chris Royse
Date: April 2026
Status: Living document — synthesizes a six-day engineering sprint, twelve session SITREPs, and seventeen subsystem reference documents into a single architectural narrative.

Abstract
This paper describes ClipCannon — an AI-driven video understanding, editing, rendering, voice-cloning, and real-time avatar pipeline — and the Teleological Constellation System that sits at its conceptual center. ClipCannon is implemented as a Model Context Protocol (MCP) server exposing 58 tools backed by a 23-stage analysis DAG, a tamper-evident provenance chain, an HMAC-secured credit billing system, and two companion systems: Phoenix, a GPU-native avatar engine, and VoiceAgent, a real-time conversational agent and meeting bot. On top of this foundation I introduce Teleological Constellation Training (TCT): a method for aligning generative video models to a target identity by constructing a "constellation" of frozen centroid vectors across seven perceptual modalities, conditioning a LoRA-fine-tuned video diffusion model on those centroids, organizing behavior into a four-level hierarchy (Action Units → Groups → Skills → Constellations), and validating every generated frame against the constellation at runtime. I document the full architecture, the six-day engineering journey through seventeen failed approaches that led to it, and argue that defining alignment as proximity to a multi-dimensional identity manifold rather than as scalar reward maximisation is a verifiable, quantitative framework with implications well beyond the talking-head avatar problem.

Table of Contents
1.	Executive Summary
2.	Vision and Motivation
3.	Part I — ClipCannon: The Foundation Pipeline
   - 3.1 System Overview
   - 3.2 Architecture and Processes
   - 3.3 The 23-Stage Analysis DAG
   - 3.4 Database Schema and Storage Tiering
   - 3.5 Provenance and the Tamper-Evident Chain
   - 3.6 The 58 MCP Tools
   - 3.7 Editing Engine
   - 3.8 Rendering Engine
   - 3.9 Audio Engine
   - 3.10 Voice Cloning Engine
   - 3.11 Avatar / Lip-Sync Engine
   - 3.12 Discovery and Narrative Intelligence
   - 3.13 Billing and the License Server
   - 3.14 The Dashboard
   - 3.15 GPU Management
   - 3.16 Test Suite and Verification
4.	Part II — The Teleological Constellation
   - 4.1 Definition
   - 4.2 The Seven Perceptual Modalities
   - 4.3 Constellation Construction from the Santa Dataset
   - 4.4 Geometric Properties of the Manifold
   - 4.5 Why "Teleological"
5.	Part III — The Four-Level Expression Hierarchy
   - 5.1 Level 0 — FLAME → FACS Action Units
   - 5.2 Level 1 — Micro-Expression Groups
   - 5.3 Level 2 — Expression Skills
   - 5.4 Level 3 — Behavioural Constellations
   - 5.5 The Constellation Controller
6.	Part IV — The Constellation Guard
   - 6.1 Architecture
   - 6.2 Modalities and Thresholds
   - 6.3 Validation Workflow
   - 6.4 Integration with Generation
7.	Part V — The Phoenix Avatar Engine
   - 7.1 Why Phoenix Exists
   - 7.2 The Render Stack
   - 7.3 PhysicsFaceEngine
   - 7.4 GPU Guard for WSL2 Stability
   - 7.5 The Constellation Subsystem
8.	Part VI — The Clone Video Pipeline
   - 8.1 End-to-End Architecture
   - 8.2 Data Preparation
   - 8.3 LoRA Training on EchoMimicV3
   - 8.4 Mouth-Weighted Loss and the Flow-Matching Sign Bug
   - 8.5 Inference and Multi-Chunk Generation
9.	Part VII — VoiceAgent and the Real-Time Meeting Bot
   - 9.1 Architecture
   - 9.2 Latency Budget
   - 9.3 The Meeting Bot Subsystem
   - 9.4 Reasoning Controller and Avatar Command
10.	Part VIII — Six Days, Seventeen Approaches
    - 10.1 Why a Negative-Result Section Belongs in a White Paper
    - 10.2 The Approaches, In Order
    - 10.3 Cross-Cutting Discoveries
    - 10.4 What Survived
11.	Part IX — Theoretical Framing
    - 11.1 Alignment as Geometric Constraint
    - 11.2 Verifiability Without Human Feedback
    - 11.3 Generalization Beyond Avatars
    - 11.4 Limitations and Open Problems
12.	Part X — System Inventory
    - 12.1 Source Code Layout
    - 12.2 Dependencies
    - 12.3 Trained Models on Disk
    - 12.4 Test Coverage
    - 12.5 Forensic Reports
13.	Conclusion
14.	Appendix A — Glossary
15.	Appendix B — Constellation Definitions
16.	Appendix C — Pipeline Stage Reference

1. Executive Summary
ClipCannon began as a video-understanding pipeline that ingests a source video, runs it through twenty-three analysis stages (probe, VFR normalization, audio extraction, source separation, frame extraction, transcription, visual embedding, OCR, scene analysis, semantic embedding, narrative analysis, prosody analysis, speaker embedding, emotion embedding, reactions, acoustic, beats, chronemic, profanity, highlights, storyboard, and finalize), and stores everything in a per-project SQLite database with vector tables backed by sqlite-vec. Around that core grew an editing engine that compiles declarative Edit Decision Lists (EDLs), a rendering engine with seven platform-optimized NVENC profiles, an audio engine that produces music in three tiers (AI diffusion, MIDI presets, DSP synthesis), a voice-cloning engine built on Qwen3-TTS with multi-gate verification, and an avatar lip-sync engine using LatentSync 1.6. All of this is exposed to AI assistants through 58 MCP tools and a tamper-evident provenance chain that links every operation back to the source via SHA-256 hashes.
On top of that foundation I built Phoenix, a GPU-native avatar engine, and VoiceAgent, a real-time conversational agent that can attend Google Meet and Zoom meetings as an AI clone. The most ambitious component of Phoenix is the Teleological Constellation System: a method for cloning a specific person from sixteen minutes of interview video, training the resulting clone to be controllable through eight named emotional states, and probably constraining its outputs to the identity manifold defined by seven embedding modalities.
The constellation system is the answer to a question I kept asking during the 24-hour and 30-hour and 16-hour and four-day and six-day sprints documented in the SITREPs: what is alignment, mechanically, for a system that has to look and sound and move like a specific human being? My answer is that alignment is distance from a frozen centroid in a multi-modal embedding space. If a generated frame, when re-embedded through the same perception model used to build the centroid, falls within a configurable cosine threshold, it is "in distribution" with the identity. If it falls outside, it is rejected. This is verifiable, quantitative, requires no ongoing human feedback, and produces a system that orbits the target identity as its telos — its purpose-defined end state.
This paper documents the full architecture as it stands today, the failures that shaped it, and the conceptual framework that holds it together.

2. Vision and Motivation
The vision is simple to state and difficult to execute clone any human being from a small amount of video so completely that an AI agent driven by that clone is indistinguishable from the original — visually, vocally, behaviorally, and conversationally — and deploy that clone as a real-time participant in meetings, chats, and content creation workflows.
That single goal generates a long list of subordinate problems: how do you understand a video in enough detail to extract a person's behavioral vocabulary? How do you separate the speaker from the interviewer? How do you capture not just what they say but how they say it — the prosody, the timing, the micro-expressions that come with each emotion? How do you train a generative video model to reproduce that person specifically without losing them in the average of everyone else in the training distribution? How do you validate that the model is still producing them and not drifting toward some generic talking head? How do you deliver the result to a meeting application in real time, with audio that sounds like them and lip movements that track their phonemes?
ClipCannon solves the first half of those problems — the understanding, editing, and asset-generation half. The Teleological Constellation System solves the second half — the identity-aligned generation and runtime validation half. They are designed as a single conceptual unit, even though they are implemented as two packages (src/clipcannon and src/phoenix) and distributed in the same wheel.
The motivation for the constellation framing in particular was practical. After failing to clone Santa convincingly with seven different generative-video architectures over six days, I realized that the missing ingredient was not a bigger model or more training data — it was a definition of what "still being Santa" meant that the system could check at every step. Without that definition, the model could and did drift: into blurry blobs, into pink amorphous masses, into other people, into generic Santas, into open-mouthed lip-flap that didn't match the audio. With the definition — the constellation — I could write loss functions that pulled toward it, attention layers that conditioned on it, and runtime guards that rejected anything that left it. The constellation gave the system a purpose.

3. Part I — ClipCannon: The Foundation Pipeline
3.1 System Overview
ClipCannon is a Python package (src/clipcannon/, 247 source files, approximately 67,585 lines of code) that exposes a video understanding, editing, rendering, audio, voice, and avatar pipeline as an MCP server. An AI assistant connects to ClipCannon over MCP, sends tool calls to ingest and analyse a source video, and receives transcripts, embeddings, scene maps, narrative analysis, prosody data, edit decision lists, rendered clips, generated music, cloned voices, and lip-synced talking-head video — all of which are stored in a per-project SQLite database with full provenance.
The package is BSL-1.1 licensed, requires Python ≥3.12, builds with Hatchling, and ships with three top-level packages in the wheel: clipcannon, license_server, and phoenix. A fourth package, voiceagent, is distributed separately as a standalone deployment.
3.2 Architecture and Processes
ClipCannon runs as three independent processes:
Process	Port	Technology	Purpose	
MCP server	stdio (or SSE/3366)	mcp SDK + Python asyncio	Exposes 58 tools to AI assistants via the Model Context Protocol	
License server	3100	FastAPI + Uvicorn	Credit billing, HMAC balance integrity, Stripe webhooks, transaction history	
Dashboard	3200	FastAPI + static HTML	Web UI for credits, projects, provenance, editing, review, and timeline visualization	

The MCP server entry point is clipcannon.server:main, defined in pyproject.toml under [project.scripts]. It sets up structured JSON logging to stderr, creates an MCP Server, registers all 58 tools by combining ALL_TOOL_DEFINITIONS and TOOL_DISPATCHERS from fourteen tool modules, and runs asyncio.run(run_stdio()). The license server and dashboard are launched separately via uvicorn.
3.3 The 23-Stage Analysis DAG
The analysis pipeline is a directed acyclic graph of twenty-three stages, six of which are required and seventeen of which are optional. Required-stage failure aborts the pipeline; optional-stage failure is logged and downstream stages depending only on optional ancestors continue. The DAG is built in src/clipcannon/pipeline/registry.py, sorted topologically via Kahn's algorithm in src/clipcannon/pipeline/dag.py, and executed level-by-level by PipelineOrchestrator in src/clipcannon/pipeline/orchestrator.py. Within each level, stages run concurrently via asyncio.gather().
The required stages are: probe, vfr_normalize, audio_extract, frame_extract, transcribe, and finalize. The optional stages are: source_separation, visual_embed, ocr, quality, shot_type, storyboard, scene_analysis, semantic_embed, narrative_llm, prosody_analysis, speaker_embed, emotion_embed, reactions, acoustic, profanity, chronemic, and highlights.
The execution order resolved by topological sort is approximately:
Level 0: probe
Level 1: vfr_normalize
Level 2: audio_extract, frame_extract
Level 3: source_separation, visual_embed, ocr, quality, storyboard,
         transcribe, acoustic
Level 4: shot_type, scene_analysis, semantic_embed, narrative_llm,
         speaker_embed, emotion_embed, reactions, profanity
Level 4.5: prosody_analysis (depends on source_separation and transcribe)
Level 5: chronemic
Level 6: highlights
Level 7: finalize

Each stage produces one or more provenance records and optionally writes to the analysis database. Sixteen of the stages are tracked in the stream_status table, providing a uniform progress interface for the dashboard. Three stages — scene_analysis, narrative_llm, and prosody_analysis — create their own dedicated tables on demand rather than appearing in stream_status.
A small selection of stage detail:
•	probe runs ffprobe, validates the source format against a whitelist (mp4, mov, mkv, webm, avi, ts, mts), detects variable frame rate, computes the source SHA-256, and updates the project table.
•	vfr_normalize snaps to the nearest standard frame rate from [23.976, 24, 25, 29.97, 30, 50, 59.94, 60] and re-encodes to constant frame rate via h264_nvenc with libx264 fallback when VFR is detected.
•	transcribe runs WhisperX large-v3 with anti-hallucination filtering (35 known phrases blocked), confidence thresholds, and tuned VAD parameters.
•	scene_analysis detects SSIM-based scene boundaries with an 8-second maximum scene duration, runs face detection per scene, identifies webcam overlay regions and content areas, and pre-computes canvas regions for layouts A (30/70 split), B (40/60 split), C (PIP), and D (full-screen face). Results go into the scene_map table.
•	narrative_llm runs Qwen3-8B in a subprocess and produces story beats, open loops, chapter boundaries, key moments, and a narrative summary. Results go into the narrative_analysis table.
•	prosody_analysis extracts F0 contour, energy, speaking rate, pitch variation, pause ratios, and a composite expressiveness score per sentence from the vocal stem, then classifies each segment by pitch contour type and energy level. Results go into the prosody_segments table and are used by the voice cloning engine for automatic reference clip selection.
•	highlights scores moments using a multi-signal blend of emotion, reaction, semantic similarity, visual quality, speaker diarisation, and narrative position.
3.4 Database Schema and Storage Tiering
Every ClipCannon project lives under ~/.clipcannon/projects/{project_id}/, with the directory layout:
proj_a1b2c3d4/
    analysis.db          # SQLite, schema version 4, all analysis results
    source/              # Original source video (copied on create)
    stems/               # Audio stems from source separation
    frames/              # Extracted frames
    storyboards/         # Contact sheet images
    edits/               # Edit working directories
    renders/             # Rendered output video files

The database uses schema version 4 and contains 34 core tables plus 4 vector tables. The four schema migrations are:
•	v1 — Phase 1 core tables (transcripts, scenes, speakers, emotion curve, topics, highlights, reactions, silence gaps, acoustic, music sections, beats, on-screen text, profanity events, content safety, pacing, storyboard grids, stream status, provenance).
•	v2 — Phase 2 editing/rendering tables (edits, edit_segments, renders, audio_assets, edit_versions, segment_cache).
•	v3 — Phase 3 voice profiles table (in a separate ~/.clipcannon/voice_profiles.db, containing 2048-dim Qwen3-TTS ECAPA-TDNN speaker embeddings).
•	v4 — Phase 4 mouth_frames table (the MouthMemory lip-sync retrieval atlas storing per-frame face/mouth crops, landmarks, viseme/phoneme labels, head pose, mouth metrics, energy, F0, emotion, and quality scores).
In addition, three tables are created on demand by their respective pipeline stages: scene_map (by scene_analysis), narrative_analysis (by narrative_llm), and prosody_segments (by prosody_analysis).
The four sqlite-vec virtual tables — vec_frames (float[1152]), vec_semantic (float[768]), vec_emotion (float[1024]), vec_speakers (float[512]) — provide K-nearest-neighbour search over visual, semantic, emotion, and speaker embeddings respectively.
Every connection applies a uniform set of pragmas: journal_mode=WAL, synchronous=NORMAL, cache_size=-64000 (64 MB page cache), foreign_keys=ON, and temp_store=MEMORY. Connections also attempt to load the sqlite-vec extension when enable_vec=True.
Disk usage is classified into three storage tiers, each with a different retention policy:
Tier	Files	Policy	
Sacred	analysis.db, WAL/SHM, everything in source/	Never auto-deleted	
Regenerable	source_cfr.mp4, stems/, frames/, storyboards/	Can be recreated by re-running pipeline stages	
Ephemeral	Logs, temporary files	Safe to delete anytime	

The clipcannon_disk_status and clipcannon_disk_cleanup MCP tools expose this classification, freeing ephemeral files first, then regenerable files (largest first), and never touching sacred files.
3.5 Provenance and the Tamper-Evident Chain
Every pipeline operation produces a provenance record in the provenance table. Each record has 22 columns including record_id (auto-generated as prov_NNN), project_id, timestamp_utc, operation, stage, input_sha256, output_sha256, model name/version/quantization/parameters, execution duration, GPU device, peak VRAM, parent_record_id, and chain_hash.
The chain hash is computed as:
SHA256(
  "{parent_hash}|{input_sha256}|{output_sha256}|{operation}|"
  "{model_name}|{model_version}|{json.dumps(model_params, sort_keys=True)}"
)

The first record uses GENESIS_HASH = "GENESIS" as parent_hash. Each subsequent record incorporates its parent's chain hash, so any modification to any historical record invalidates all downstream chain hashes. The verify_chain function walks the entire chain in chronological order, recomputes each hash from its parent, and compares against the stored value, returning a ChainVerificationResult that identifies the broken record (if any) along with a human-readable issue description.
This design detects accidental corruption and unauthorised partial edits. It does not detect a privileged actor with database write access who recomputes the entire chain from genesis with modified data — that would require digital signatures, which are intentionally not part of this system. The threat model is integrity verification, not adversarial tamper resistance.
The sha256_table_content function provides a deterministic hash of all rows in a table for a given project by sorting rows by their JSON-serialised form before hashing, allowing the chain to incorporate complete table snapshots when needed.
3.6 The 58 MCP Tools
ClipCannon exposes 58 tools to AI assistants via the MCP protocol. Their dispatch is centralised through src/clipcannon/tools/__init__.py, which combines tool definitions and dispatchers from fourteen modules. The clipcannon.server:main function registers list_tools() to return ALL_TOOL_DEFINITIONS and call_tool(name, arguments) to look up TOOL_DISPATCHERS[name]. Tool results are serialised to JSON TextContent; tools that return image data also produce ImageContent (base64-encoded JPEG).
The 58 tools are organised into thirteen groups:
Group	Count	Examples	
Project management	5	clipcannon_project_create, clipcannon_project_status, clipcannon_project_delete	
Understanding	4	clipcannon_ingest, clipcannon_get_transcript, clipcannon_get_frame, clipcannon_search_content	
Discovery	4	clipcannon_find_best_moments, clipcannon_find_cut_points, clipcannon_get_narrative_flow, clipcannon_find_safe_cuts	
Disk management	2	clipcannon_disk_status, clipcannon_disk_cleanup	
Configuration	3	clipcannon_config_get, clipcannon_config_set, clipcannon_config_list	
Editing	11	clipcannon_create_edit, clipcannon_modify_edit, clipcannon_auto_trim, clipcannon_apply_feedback, clipcannon_branch_edit	
Rendering	8	clipcannon_render, clipcannon_get_editing_context, clipcannon_inspect_render, clipcannon_preview_layout, clipcannon_get_scene_map	
Audio generation	6	clipcannon_generate_music, clipcannon_compose_midi, clipcannon_generate_sfx, clipcannon_audio_cleanup, clipcannon_auto_music, clipcannon_compose_music	
Voice	4	clipcannon_prepare_voice_data, clipcannon_voice_profiles, clipcannon_speak, clipcannon_speak_optimized	
Avatar	2	clipcannon_lip_sync, clipcannon_extract_webcam	
End-to-end video generation	1	clipcannon_generate_video	
Constellation / clone video	4	clipcannon_list_constellations, clipcannon_list_skills, clipcannon_clone_video, clipcannon_expression_sequence	
Billing	4	clipcannon_credits_balance, clipcannon_credits_history, clipcannon_credits_estimate, clipcannon_spending_limit	

The constellation tools are the externally visible surface of the Teleological Constellation System. The clipcannon_clone_video tool in particular generates a complete clone avatar video from scratch using EchoMimicV3 with LoRA identity lock and constellation-controlled expressions, and is distinct from clipcannon_generate_video (which uses LatentSync to lip-sync onto an existing driver video).
3.7 Editing Engine
The editing engine (src/clipcannon/editing/) is built around an Edit Decision List data model in edl.py. The top-level EditDecisionList Pydantic model contains an edit ID, project ID, name, status, source SHA-256, target platform, target profile, segments, captions, crop, canvas, audio, metadata, and render settings. Segments support per-segment canvas overrides with compositing regions and animated zoom (start scale → end scale with configurable easing).
Validation in validate_edl checks the source SHA-256 against the project, ensures segments are non-overlapping and within source duration, enforces speed range 0.25–4.0, and verifies transition durations. There are no platform duration limits — duration is unconstrained by design to support real-world workflows.
Caption generation in captions.py chunks transcript words into adaptive caption segments: maximum three words per chunk, breaks at sentence punctuation and commas, adapts to speech rate (fast >200 WPM reduces max words, slow <80 WPM increases), and merges chunks separated by gaps under 200 ms. The caption_render.py module generates ASS subtitle files in four styles (bold_centered, word_highlight, subtitle_bar, karaoke) with Script Info, V4+ Styles, and Events sections.
Smart cropping in smart_crop.py uses MediaPipe (primary) or InsightFace (fallback) for face detection, computes crop regions centred on faces with an 85% safe area, supports cover/contain/stretch fit modes, and offers split-screen, picture-in-picture, and full-canvas compositing layouts. The measure_layout.py module pre-computes precise canvas regions for layouts A through D on a 1080×1920 canvas.
The engine also supports automated filler/pause trimming (auto_trim.py), color grading (ColorSpec in EDL), motion effects (motion.py: zoom in/out, pan in four directions, Ken Burns), visual overlays (overlays.py: lower thirds, title cards, logos, watermarks, CTAs), iterative editing with full version control (edit_versions table, revert_edit tool, branching via parent_edit_id), natural-language feedback application via apply_feedback, and segment-level render caching keyed by content hash to avoid re-rendering unchanged segments.
3.8 Rendering Engine
The rendering engine (src/clipcannon/rendering/) is an async FFmpeg-based pipeline that compiles an EDL to a platform-optimised MP4. The seven encoding profiles cover TikTok vertical, Instagram Reels, YouTube Shorts, YouTube Standard, YouTube 4K, Facebook, and LinkedIn, all using h264_nvenc with software libx264 fallback. Bitrates range from 5 Mbps (LinkedIn 1:1) to 40 Mbps (YouTube 4K 16:9 at 3840×2160).
The render pipeline performs:
17.	Source resolution with SHA-256 verification (and rejection of /renders/ sources to prevent generation loss).
18.	Profile resolution.
19.	Caption file generation (ASS).
20.	Crop computation from EDL and scene data.
21.	FFmpeg command building dispatched by layout: canvas (per-segment regions), split screen, picture-in-picture, single segment, or multi-segment with xfade transitions.
22.	Async FFmpeg subprocess execution.
23.	Thumbnail generation (JPEG -q:v 2, ~95% quality).
24.	Provenance record linking the rendered file to the source.
25.	Database update to the renders table.
Per-segment canvas compositing supports independent regions per segment with source crop, output placement, z-index, fit mode, opacity, border, and animated zoom. EDL transition types are mapped to FFmpeg xfade modes (fade → fade, dissolve → dissolve, wipe_left/right → wipeleft/wiperight, slide_left/right → slideleft/slideright, zoom_in → smoothup).
The clipcannon_render tool charges 2 credits, refunded on failure. The clipcannon_inspect_render tool extracts frames at 0/25/50/75/100% timestamps, probes metadata via ffprobe, and runs quality checks. The clipcannon_preview_clip and clipcannon_preview_segment tools produce 540p previews at no credit cost. Batch rendering in batch.py uses an asyncio semaphore to limit concurrent FFmpeg/NVENC sessions to rendering.max_parallel_renders.
3.9 Audio Engine
The audio engine (src/clipcannon/audio/) is organised into three tiers, all producing WAV at 44 100 Hz:
•	Tier 1 (AI): ACE-Step v1.5 turbo RL diffusion (music_gen.py, GPU, 4+ GB VRAM, with CPU offload below 8 GB) and Meta MusicGen via AudioCraft (musicgen.py, native 32 kHz resampled to 44.1 kHz, with windowed generation and 5 s overlap crossfade for durations over 30 s).
•	Tier 2 (Synth): MIDI composition (midi_compose.py) from twelve presets — ambient_pad, upbeat_pop, corporate, dramatic, minimal_piano, intro_jingle, lofi_chill, cinematic_epic, tech_corporate, acoustic_folk, synth_wave, jazz_smooth — with theory-correct chord progressions, melody patterns, optional bass, and optional drums. MIDI is rendered to WAV via FluidSynth (midi_render.py) with a search across five common SoundFont locations. LLM-driven planning in midi_ai.py translates natural-language descriptions to structured MIDI parameters via Qwen3-8B.
•	Tier 3 (DSP): Thirteen programmatic sound effects (sfx.py) — whoosh, riser, downer, impact, chime, tick, bass_drop, shimmer, stinger, ambient_drone, ambient_texture, pad_swell, nature_bed — implemented in pure NumPy/SciPy with zero-crossing fades, normalisation, and 16-bit WAV output.
Mixing in mixer.py uses speech-aware ducking: RMS energy analysis in 50 ms windows identifies speech regions and reduces music by duck_level_db (-6 dB default) with 200 ms attack and 300 ms release ramps. Effects in effects.py use Spotify pedalboard for reverb, compression, EQ low/high cut, and limiting. Cleanup in cleanup.py performs noise reduction (spectral gating), de-hum (notch filters at 50/60 Hz and harmonics), de-ess, and loudness normalization.
The music_planner.py module produces a MusicBrief from a project's emotion curve, pacing, beats, and silence gaps, suggesting overall mood, energy level, tempo, key, preset, and an ACE-Step prompt. This brief drives the clipcannon_auto_music tool, which generates background music tailored to the edit without manual prompting.
3.10 Voice Cloning Engine
The voice engine (src/clipcannon/voice/) clones a person's voice from their existing audio in a multi-stage pipeline:
26.	Data preparation (data_prep.py) — extracts vocal stems from ingested projects, splits at silence boundaries, matches splits with the transcript, phonemises the matched text, and writes train/val manifests. It also exports prosody features so reference clips can be selected by delivery style.
27.	Profile management (profiles.py) — SQLite CRUD over the voice_profiles table in ~/.clipcannon/voice_profiles.db. Each profile stores a 2048-dimensional Qwen3-TTS ECAPA-TDNN speaker embedding and a verification threshold.
28.	Synthesis (inference.py) — VoiceSynthesizer calls Qwen3-TTS with the reference embedding, runs an iterative multi-gate verification loop (sanity → intelligibility → identity), and returns the best result.
29.	Verification (verify.py) — VoiceVerifier runs three gates: sanity (duration, clipping, SNR, leading silence), intelligibility (Whisper-based WER with punctuation stripping), and identity (Speaker Encoder Cosine Similarity, SECS).
30.	Optimized synthesis (optimize.py) — optimized_speak generates N candidates and returns the highest-SECS one, then post-processes with Resemble Enhance.
31.	Enhancement (enhance.py) — Resemble Enhance denoise plus latent flow matching up samples 24 kHz TTS output to 44.1 kHz broadcast quality.
32.	Prosody-aware reference selection (prosody_select.py) — automatically picks reference clips matching the target delivery style (energetic, calm, emphatic, varied, fast, slow, rising, question, best) by querying the prosody_segments table.
33.	Multi-voice synthesis (multi_voice.py) — swaps between profiles mid-conversation for dialogue scenarios.
34.	Prosody backfill (prosody_backfill.py) — backfills prosody features into prosody_segments for existing voice training clips that pre-date the prosody analysis stage.
The hard quality floor — SECS ≥ 0.95 at all times, full prosody, no quality trade-offs for speed — is enforced by the verification gates. The user has explicitly required this floor and it is a non-negotiable behavioral rule.
3.11 Avatar / Lip-Sync Engine
The avatar engine (src/clipcannon/avatar/) wraps LatentSync 1.6 (ByteDance) — a diffusion-based lip-sync pipeline using a VAE, UNet3D, and DDIM scheduler at 512×512 resolution. The LipSyncEngine class loads the model lazily, takes audio plus a driver video, runs configurable inference steps with a guidance scale, supports multi-candidate selection (n_candidates 1 for speed, 3-5 for quality), and returns a LipSyncResult. The clipcannon_extract_webcam tool extracts the webcam/face region from an ingested video as a standalone driver video using scene_map face data, eliminating the need to crop manually.
The clipcannon_generate_video tool composes voice synthesis and lip-sync into a single end-to-end call: text script → cloned voice → lip-synced video.
3.12 Discovery and Narrative Intelligence
The discovery tools (src/clipcannon/tools/discovery.py) provide cross-stream intelligence over the analysis database:
•	clipcannon_find_best_moments — purpose-aware scoring (hook, highlight, CTA, tutorial step) with cross-stream convergence analysis.
•	clipcannon_find_cut_points — natural edit boundaries from convergence of silence, beat, scene, and sentence boundaries.
•	clipcannon_get_narrative_flow — narrative coherence analysis of proposed segments before rendering.
•	clipcannon_find_safe_cuts — audio-safe cut points across the entire video.
The clipcannon_get_editing_context rendering tool returns transcript, ranked highlights, silence gaps, pacing, scene boundaries, and narrative analysis in a single call, intended to be the first tool an AI assistant calls before creating an edit.
3.13 Billing and the License Server
The billing system uses a three-layer trust model: MCP tools and the dashboard talk over HTTP localhost to a FastAPI license server on port 3100, which stores HMAC-signed balances in SQLite at ~/.clipcannon/license.db, with a planned (currently stubbed) Cloudflare D1 sync for remote source-of-truth.
Balance integrity uses HMAC-SHA256(machine_key, "balance:{balance}") with a deterministic per-machine key derived from SHA256("{hostname}|{mac}|{cpu_arch}")[:32]. A versioned salt ("clipcannon-v1|{machine_id}") enables future key rotation. Verification uses constant-time comparison via hmac.compare_digest. On mismatch the system raises BillingError(BALANCE_TAMPERED) and returns HTTP 403. There is no recovery path — balance tampering is fatal.
The credit rates are:
Operation	Credits	
analyze	10	
render	2	
metadata	1	
publish	1	

Credit packages are starter (50/$5), creator (250/$20), pro (1000/$60), and studio (5000/$200). Dev mode initializes with 100 credits and a 200-credit monthly spending limit. Spending limits trigger warnings at 80% of cap, and operations exceeding the cap are blocked with SPENDING_LIMIT_EXCEEDED.
The license server exposes endpoints /health, /v1/charge, /v1/refund, /v1/balance, /v1/history, /v1/sync, /v1/add_credits, and /v1/spending_limit. Charges are idempotent via an idempotency_key. Stripe webhook handling lives in stripe_webhooks.py and processes checkout.session.completed events.
3.14 The Dashboard
The dashboard (src/clipcannon/dashboard/) is a FastAPI application on port 3200 with seven route groups: home, credits, projects, provenance, editing, review, and timeline. JWT-based dev-mode authentication uses HS256 with a 30-day TTL stored in an HTTP-only cookie. The dashboard mounts static/ last so API routes take precedence, and falls back to JSON responses when no static HTML exists.
The provenance endpoints (/api/provenance/{project_id}, /api/provenance/{project_id}/verify, /api/provenance/{project_id}/timeline) expose chain inspection and verification through the web UI. The timeline endpoint produces an aggregated visualisation combining scenes, speakers, emotion curve, topics, and highlights. The review queue lets editors batch-approve or batch-reject rendered edits.
3.15 GPU Management
GPU management is split between a general-purpose ModelManager in src/clipcannon/gpu/ and a stricter gpu_guard in src/phoenix/video/. The two are designed for different workloads: the ModelManager handles ClipCannon's pipeline stages with LRU model lifecycle and concurrent-vs-sequential mode based on VRAM, while the Phoenix gpu_guard enforces hard VRAM budgets and serializes GPU operations through a threading lock specifically tuned for RTX 5090 + WSL2 stability.
Precision auto-detection in gpu/precision.py maps CUDA compute capability to a default precision: 12.0 (Blackwell) → nvfp4, 8.9 (Ada Lovelace) → int8, 8.6 (Ampere) → int8, 7.5 (Turing) → fp16, no GPU → fp32. The validate_gpu_for_pipeline function returns availability, device name, compute capability, total VRAM, default precision, and a minimum-VRAM check (≥8 GB).
The ModelManager enters concurrent mode when total VRAM exceeds 16 GB, keeping multiple models loaded with LRU eviction. Below 16 GB it switches to sequential mode, unloading every model before loading a new one.
The Phoenix gpu_guard is more aggressive. Before any torch import it sets PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True,max_split_size_mb:512,garbage_collection_threshold:0.6, CUDA_MODULE_LOADING=LAZY, and HF_HUB_OFFLINE=1/TRANSFORMERS_OFFLINE=1 to forbid runtime model downloads. Blackwell tensor core optimizations enable TF32 matmul, cuDNN benchmark, and float32_matmul_precision="high". The VRAM budget for the RTX 5090 is 32 768 MB total, 4 096 MB reserved (driver and OS), and 28 672 MB usable. The gpu_operation context manager acquires a threading lock so only one GPU operation runs at a time, preventing the WSL2 CUDA context contention crashes that derailed multiple training runs documented in the SITREPs.
3.16 Test Suite and Verification
Pytest collects 1 131 tests across 85 test files. Test counts are split between core ClipCannon, VoiceAgent, and Phoenix. The test categories include EDL validation, captioning, smart cropping, encoding profiles, audio generation, billing, HMAC integrity, provenance hash chain, full-pipeline integration with a real video, voice verification, lip-sync, expression skills, constellation controller, constellation guard, physics-face engine, and CUDA optimizations.
In addition to unit and integration tests, there are ten Full State Verification (FSV) scripts that perform forensic, end-to-end checks. The four primary FSV scripts together run 750 individual checks covering core infrastructure (154), pipeline plus tools (189), billing plus dashboard (84), and server integration (323). Six extended FSV scripts add additional coverage. All FSV scripts default to assuming code is guilty until proven innocent and require evidence (database state, file existence, hash verification) for every PASS.
Ruff linting is configured with rules E, F, W, I, N, UP, ANN, B, SIM, TCH for Python 3.12 with line length 100.

4. Part II — The Teleological Constellation
4.1 Definition
A Teleological Constellation for a target identity P is a set of frozen centroid vectors:
C = { (m, c_m) | m ∈ M }

where M is the set of perceptual modalities and c_m is the L2-normalised centroid of all embeddings extracted from reference video of P in modality m. Given a collection of embeddings E_m = {e_m^1, ..., e_m^N}, the centroid is computed as:
c_m = normalize(mean(normalize(e_m^i)))

When behavioral states are also defined (warm conversational, emotional recall, happy storytelling, etc.) the constellation extends to C = { (m, s, c_ms) } where each centroid is computed only over embeddings classified into state s.
The constellation is frozen once constructed. It is the target against which both training and inference are oriented. Loss functions pull the model toward it, attention layers condition on it, and the runtime guard rejects outputs that fall outside its neighbourhood.
4.2 The Seven Perceptual Modalities
The constellation spans seven modalities, totalling 4 044 dimensions:
#	Modality	Model	Dimensions	Captures	
1	Visual	SigLIP-SO400M-patch14-384	1 152	Facial appearance, pose, expression	
2	Semantic	Nomic-embed-text-v1.5	768	Spoken content, topics, vocabulary	
3	Emotion	Wav2Vec2-large-emotion	1 024	Vocal emotion, arousal, valence	
4	Speaker	WavLM-large	512	Voice identity, timbre, formants	
5	Prosody	Custom (F0, energy, rate, contour)	12	Speaking style	
6	Sentiment	all-MiniLM-L6-v2	384	Sentence-level sentiment, intent	
7	Voice	ECAPA-TDNN	192	Speaker verification embedding	

A deviation in any single modality is detectable. The visual modality catches face drift; the speaker modality catches vocal drift; the emotion modality catches affective drift; the semantic modality catches content drift; the prosody modality catches delivery drift; the sentiment modality catches intent drift; the voice modality catches verification drift. Together they form a 4 044-dimensional manifold that is dense enough to define the identity precisely and sparse enough to leave room for variation within it.
The visual encoder (SigLIP-SO400M-patch14-384) is shared between training-time embedding extraction and inference-time constellation guard validation. This is essential: the same encoder produces both the constellation centroid and the embedding of a generated frame, which means cosine similarity is meaningful and consistent.
4.3 Constellation Construction from the Santa Dataset
The reference material is a 975-second (16 minutes 15 seconds) interview with the subject ("Santa"). ClipCannon's pipeline ingests this video, extracts frames at 2 fps, transcribes via WhisperX, separates audio sources via Demucs, runs semantic and emotion and speaker embedders, and analyses prosody. The result is a complete analysis.db with thousands of rows across all the analysis tables.
A separate extraction script (scripts/extract_all_embeddings.py) consolidates the seven modalities into ~/.clipcannon/models/santa/embeddings/all_embeddings.npz with the following sample counts after diarisation filtering:
Modality	Samples	Dimensions	
Visual	1 725	1 152	
Emotion	343	1 024	
Speaker	300	512	
Sentiment	202	384	
Voice	188	192	
Prosody	188	12	
Semantic	200	768	

The constellation centroid for each modality is the L2-normalised mean of the L2-normalised samples. With 1 725 visual samples, the visual centroid is a robust estimate of the subject's face manifold across all the angles, expressions, and lighting conditions present in the interview.
4.4 Geometric Properties of the Manifold
The constellation has three properties that matter for alignment:
35.	Density. With 4 044 dimensions across seven modalities, the manifold is dense enough that drift in any one modality is detectable. The cosine similarity threshold for visual is 0.70 (meaning the generated frame must be at most ~45° from the centroid in 1 152-dimensional space), for emotion 0.60, for speaker 0.80, and for voice 0.85. Visual severity tiers add WARN at 0.80 and STRONG at 0.90.
36.	Stability. The centroid is a frozen point. It does not move during training or inference. This means the alignment target is invariant — there is no drift in the target itself, only in the model's outputs relative to it.
37.	Decomposability. Each modality contributes an independent constraint. A model that produces a visually correct face with the wrong voice fails the speaker check; a model that produces the right voice with the wrong face fails the visual check. The modalities cannot trade off against each other — they all must be satisfied simultaneously.
The manifold is also extensible. New modalities can be added (touch, gesture, gaze, full-body pose) by adding their embedders to the extraction pipeline, computing their centroids, and adding them to the guard. The architecture is open-ended.
4.5 Why "Teleological"
The word teleological means "oriented toward purpose or end-state". I chose it deliberately. In standard generative model training, the loss function is causal: we minimise prediction error on the next token, the next frame, the next pixel, in the hope that this produces good outputs. The hope is empirical and the alignment is implicit. In the constellation framing, the loss function is teleological: we minimise distance from the frozen identity centroids, making the purpose of the model — to be Santa — explicit at every gradient step. The constellation is the telos. The model orbits it.
This is more than rhetorical. A causal loss function can drift indefinitely; the constellation pulls back. A causal loss function can be satisfied by averages that please no one; the constellation rejects averages that don't fall within the centroid neighbourhood. A causal loss function offers no test-time guarantee; the constellation guard provides one. The model's architecture, training loss, and runtime validator are all organised around the same end-state, which is the definition of teleological design.

5. Part III — The Four-Level Expression Hierarchy
The constellation defines who the clone is. The expression hierarchy defines what the clone can do. It is implemented in src/phoenix/video/expression_skills.py and exposed through the clipcannon_list_constellations, clipcannon_list_skills, and clipcannon_expression_sequence MCP tools.
5.1 Level 0 — FLAME → FACS Action Units
FLAME (Faces Learned with an Articulated Model and Expressions; Li et al., 2017) is a parametric head model with 5 023 vertices, 300 shape parameters, 100 expression parameters, and 3D jaw, neck, and eye pose. ClipCannon's flame_fitter.py fits FLAME parameters to face crops at 82 frames per second on an RTX 5090, producing a 100-dimensional expression vector per frame.
The FLAME_TO_AU table in expression_skills.py maps 17 FLAME expression coefficient indices to FACS Action Units (Ekman & Friesen, 1978):
FLAME Idx	AU	Name	Region	
0	AU26	jaw_open	jaw	
1	AU12	lip_spread	mouth	
2	AU22	lip_funneler	mouth	
3	AU18	lip_pucker	mouth	
4	AU23	lip_tightener	mouth	
5	AU1+2	brow_raise	brow	
6	AU6+7	eye_squint	eye	
7	AU27	mouth_stretch	mouth	
8	AU24	lip_press	mouth	
9	AU12b	lip_corner_pull	mouth	
10	AU6	cheek_raise	cheek	
15	AU14	dimpler	cheek	
22	AU28	lip_suck	mouth	
24	AU9	nose_wrinkle	nose	
31	AU17	chin_raise	jaw	
37	AU4	brow_lower	brow	
42	AU5	eye_widen	eye	

This gives the system a human-interpretable vocabulary for describing micro-expressions. The 17 AUs cover the primary muscle groups responsible for the seven basic emotions and most secondary expressions.
5.2 Level 1 — Micro-Expression Groups
A MicroExpressionGroup is an atomic group of AUs that fire together. Groups are discovered by K-Means clustering (default 40 clusters) over the FLAME expression coefficients in the training data. Each cluster centroid defines a group; active AUs are identified by thresholding the deviation from the neutral (mean) expression at 0.5, with a fallback to the top three by magnitude. This produces a data-driven vocabulary of micro-expressions specific to the subject.
For Santa, the 40 groups represent forty distinct facial configurations he uses across the 16-minute interview — not generic expressions, but his expressions: the way his brow lifts when he's surprised, the way his jaw tenses when he's emphatic, the way his cheeks engage when he laughs. The clustering captures the individuality of his facial behaviour without requiring any manual labelling.
5.3 Level 2 — Expression Skills
An ExpressionSkill is a named behavioural expression with three temporal phases (onset, peak, offset) and a natural-language prompt for EchoMimicV3. There are 34 hard-coded skills in SKILL_PROMPTS:
genuine_laugh, warm_smile, gentle_nod, eye_contact, thoughtful_pause, gaze_down, voice_waver, brave_smile, deep_breath, eyes_wide, animated_gesture, emphatic_nod, brow_furrow, soft_eyes, slow_nod, lip_press, head_tilt, gaze_up, slight_frown, pause, slow_blink, chin_rest, rapid_speech, lean_forward, hand_gesture, steady_gaze, slow_speech, minimal_movement, slight_nod, eye_twinkle, half_smile, slight_pause, warm_chuckle, slight_gesture.
Skills are built from temporal clustering: 10-frame sliding windows of cluster labels are histogrammed and clustered again (default 25 skill clusters). The temporal phases are 0.2 s onset, 0.5 s peak, and 0.3 s offset, totalling one second per skill execution. The natural-language prompt is what gets injected into the EchoMimicV3 generation prompt during inference.
The skill_to_prompt function modulates the prompt by an intensity parameter: less than 0.3 prepends "very subtly", less than 0.6 prepends "slightly", more than 0.9 prepends "intensely". This gives the runtime controller a continuous knob over expression strength.
5.4 Level 3 — Behavioural Constellations
A BehavioralConstellation is an emotional state expressed as a cycling sequence of skills. There are eight hard-coded constellations:
Constellation	Emotion	Skills	Description	
warm_conversational	joy + trust	warm_smile, gentle_nod, eye_contact, slight_gesture	Default friendly interview state	
emotional_recall	sadness + trust	thoughtful_pause, gaze_down, voice_waver, brave_smile, deep_breath	Remembering emotional events	
happy_storytelling	joy + anticipation	genuine_laugh, eyes_wide, animated_gesture, warm_smile, emphatic_nod	Telling a happy or funny story	
empathetic_concern	sadness + trust	brow_furrow, soft_eyes, slow_nod, lip_press, head_tilt	Showing care about someone else's pain	
thoughtful_reflection	anticipation + trust	gaze_up, slight_frown, pause, slow_blink, chin_rest	Deep thinking about a topic	
excited_sharing	joy + surprise	eyes_wide, rapid_speech, lean_forward, hand_gesture, genuine_laugh	Excited to share something	
solemn_gravity	trust + sadness	steady_gaze, slow_speech, minimal_movement, slight_nod, deep_breath	Speaking about something serious	
gentle_humor	joy + surprise	eye_twinkle, half_smile, slight_pause, warm_chuckle	Mild amusement, not full laugh	

Each constellation cycles through its skills at approximately one skill every 2 seconds, producing a continuous stream of micro-expression conditioning that shapes the generated video frame by frame. A scene-by-scene reasoning layer (Gemma 4 in the meeting bot context) selects which constellation is active at any moment based on what the avatar is saying.
The SkillLibrary class encapsulates the full hierarchy and provides extract_from_training_data(embeddings_path, flame_params_path, n_groups=40, n_skills=25) to build the entire system from a subject's training data in one call.
5.5 The Constellation Controller
The ConstellationController (in src/phoenix/video/constellation_controller.py) is the runtime orchestrator. Its public API:
Method	Description	
set_emotional_state(constellation, intensity)	Set the active constellation	
queue_expression(skill, duration, at_time)	Queue a specific expression skill at a time	
get_prompt_for_frame(frame_idx, fps=25)	Get the NL prompt for a specific frame	
get_prompt_sequence(duration_s, fps=25)	Get a list of (frame_idx, prompt) for full duration	
blend_constellations(a, b, ratio)	Blend two constellations (0.0 pure a, 1.0 pure b)	
transition_to(new_constellation, duration_s=0.5)	Schedule a smooth transition between constellations	

Prompt resolution follows a strict priority order. Queued expressions take precedence (with onset/peak/offset intensity ramps: 0–20% onset, 20–70% peak, 70–100% offset). Active transitions take next priority (blended prompts). The current constellation's cycling skill sequence takes lowest priority. This means a one-shot expression queued for a specific moment will override the background cycle without disrupting it once it completes.
The clipcannon_expression_sequence MCP tool exposes get_prompt_sequence so an AI assistant can preview the per-frame prompts that will be generated for a constellation over a duration before committing to a full generation pass.

6. Part IV — The Constellation Guard
6.1 Architecture
The Constellation Guard is the runtime validator that closes the alignment loop. It is implemented in src/phoenix/video/constellation_guard.py and operates as follows:
Generated frame
    │
    ▼
Re-embed through frozen SigLIP-SO400M-patch14-384 (1152-d)
    │
    ▼
Cosine similarity vs. constellation centroid
    │
    ▼
Compare to per-modality threshold
    │
    ▼
Accept or reject (with rejection_reason)

The same encoder used to build the visual centroid is used to embed every generated frame. This means the comparison is meaningful: it answers the question "does this frame, when embedded through the same encoder, fall within distance D of the centroid?" If yes, it is in distribution with the identity. If no, the frame is rejected and the system either re-generates or falls back to a known-good frame.
The constellation guard is modality-agnostic. The visual modality is validated frame-by-frame at inference time. The audio modalities (speaker, voice) are validated against the audio embedding produced for each generated chunk. The semantic and emotion modalities are used during training-time loss computation.
6.2 Modalities and Thresholds
Constellation centroids are loaded from all_embeddings.npz with the following modality keys:
Modality	NPZ key	Description	
visual	vis_emb	SigLIP visual embeddings	
semantic	sem_emb	Semantic embeddings	
emotion	emo_emb	Emotion embeddings	
speaker	spk_emb	Speaker embeddings	
prosody	pro_data	Prosody features	
sentiment	sent_emb	Sentiment embeddings	
voice	voice_emb	Voice embeddings	

The default cosine-similarity thresholds are:
Modality	Threshold	Severity tiers (visual only)	
Visual	0.70	WARN at 0.80, STRONG at 0.90	
Emotion	0.60	-	
Speaker	0.80	-	
Voice	0.85	-	

These are conservative starting points, configurable per deployment. The voice and speaker thresholds are higher because vocal identity is a more sensitive signal than visual identity (a face can change with lighting and angle; a voice cannot).
6.3 Validation Workflow
The guard exposes the following methods:
Method	Description	
load_visual_encoder()	Load frozen SigLIP (~2 GB VRAM). Uses gpu_guard.check_vram. Requires HF_HUB_OFFLINE=1.	
validate_frame(frame_rgb)	Single-frame validation: embed and compare	
validate_batch(frames)	Batched validation in a single forward pass	
validate_audio(audio_embedding, modality)	Audio validation against speaker/voice centroids	
get_constellation_stats()	Centroid dimensions, sample counts, spread, thresholds	
unload()	Free SigLIP and GPU memory	

Each call returns a ValidationResult with valid (bool), visual_similarity (float), details (dict mapping modality to similarity), and rejection_reason (string or None).
The visual encoder load uses gpu_guard.check_vram to ensure the 2 GB allocation is available before proceeding, and HF_HUB_OFFLINE=1 is enforced so the system never silently downloads a model at runtime — a hard rule documented in feedback_no_model_downloads.md after multiple incidents where transient runtime downloads caused production failures.
6.4 Integration with Generation
In the clone video pipeline, the guard runs after every chunk of generated video. If a frame fails the visual threshold, the system can regenerate the chunk with a different seed, fall back to the previous chunk's last frame, or escalate to the human operator. In the meeting bot context, the guard runs continuously on the avatar's video stream and pages the operator if a sustained drift is detected.
The guard's role is defense in depth: even if the LoRA-locked model drifts, even if the audio attention misfires, even if a hardware glitch produces garbage frames, the guard catches it before the audience sees it. The constellation is not just a training target — it is also a real-time sanity check on every frame the system produces.

7. Part V — The Phoenix Avatar Engine
7.1 Why Phoenix Exists
ClipCannon's avatar tools (clipcannon_lip_sync, clipcannon_extract_webcam) handle batch lip-sync via LatentSync. Phoenix exists to handle the harder problem: real-time, identity-locked, controllable avatar rendering for interactive scenarios like meetings, voice agents, and live streams. It is GPU-native, designed for the RTX 5090, and integrates the Teleological Constellation System as its identity layer.
Phoenix is packaged in the same wheel as ClipCannon (pyproject.toml lists src/clipcannon, src/license_server, and src/phoenix in the wheel build targets). Its four constellation MCP tools are registered through the same dispatcher that registers all other ClipCannon tools.
7.2 The Render Stack
Phoenix's render pipeline (src/phoenix/render/) is built around several specialized components:
•	`flame_model.py` — FLAME parametric head model. Provides 5 023 vertices, 100 expression coefficients, jaw and neck pose, and 3D mesh evaluation.
•	`flame_fitter.py` — FLAME fitting to face detections. Achieves 82 frames per second on an RTX 5090, producing per-frame expression parameters that feed into the constellation hierarchy.
•	`physics_face.py` — PhysicsFaceEngine. Audio formant physics → face animation parameters (see §7.3).
•	`audio_features.py` — MFCC, energy, pitch, and YIN extraction for driving animation.
•	`lip_sync.py` — Audio-driven lip animation within the rendering pipeline.
•	`temporal_filter.py` — Per-region exponential moving average for jitter-free output.
•	`webcam_effects.py` — GPU-accelerated noise, vignette, and other webcam-style effects.
•	`cupy_compositor.py` — CUDA zero-copy compositing via CuPy for full-GPU image composition.
•	`compositor_bridge.py` — Bridge between the behavior and expression systems and the GPU compositor.
•	`y4m_writer.py` — YUV4MPEG2 raw frame writer for pipeline output.
•	`_gpu_kernels.py` — Custom CUDA kernels for hot operations.
In the early sessions documented in the SITREPs, Phoenix also contained a Gaussian Splatting renderer based on gsplat, a face warper, an NVENC encoder, and a Perceiver-based clone model with North Star embeddings. After multiple training failures (the Gaussian model produced blurry blobs even at 50 000 iterations) those components were removed; the directory now retains only the FLAME, lip-sync, physics, compositing, and effects modules. The clone model training has moved to LoRA fine-tuning of EchoMimicV3 in external scripts under scripts/train_santa_lipsync.py.
7.3 PhysicsFaceEngine
PhysicsFaceEngine (src/phoenix/render/physics_face.py) is an unusual component. It produces facial animation parameters directly from audio physics — specifically formant frequencies (F1, F2, F3) extracted via Linear Predictive Coding, fundamental frequency (F0) via YIN, and energy. Each formant maps deterministically to a face parameter: F1 → jaw open, F2 → lip spread, F0 → chin tension. The whole system is sub-5 ms per chunk and works on any voice in any language with no training.
The interesting property of the engine is that it gives the system a causal model of speech-driven mouth movement that does not depend on neural networks. It is the kind of thing speech labs built in the 1980s, but it works perfectly for one specific job: producing physically plausible mouth movement that drives a face renderer. In the SITREPs the user described it as "the genuine innovation of this session" — it combines formant physics with face animation in a way that no prior system the project investigated had done.
PhysicsFaceEngine is the deterministic complement to the LoRA-trained EchoMimicV3 model. The LoRA model produces realistic talking-head video with neural-network unpredictability; PhysicsFaceEngine produces deterministic, audio-aligned mouth shapes that can be used for fast preview, fallback rendering, or as a baseline conditioning signal during training. It is also trivially testable (deterministic output for the same audio) and has 34 unit tests in tests/phoenix/test_physics_face.py.
7.4 GPU Guard for WSL2 Stability
gpu_guard.py is the safety harness for everything Phoenix does on the GPU. The guard is necessary because WSL2's CUDA driver has well-documented stability issues under sustained load: concurrent CUDA contexts can crash the entire driver, BF16 conv3d operations can crash when run while other models are loaded, and the allocator state can become corrupted after model swaps even when only a small amount of VRAM is in use.
The guard's defences:
38.	Threading lock around every GPU operation. The gpu_operation(name, estimated_vram_mb) context manager acquires _gpu_lock so only one operation runs at a time. This eliminates context contention crashes.
39.	Aggressive cleanup. gpu_cleanup() runs gc.collect, torch.cuda.synchronize, torch.cuda.empty_cache, and torch.cuda.reset_peak_memory_stats in sequence after every operation.
40.	VRAM budget enforcement. check_vram(required_mb, operation) checks the budget before allowing an operation to proceed; if insufficient, it tries cleanup first and raises RuntimeError if it still cannot fit.
41.	Pre-import environment configuration. Before the first torch import, the guard sets PYTORCH_CUDA_ALLOC_CONF, CUDA_MODULE_LOADING=LAZY, HF_HUB_OFFLINE=1, and TRANSFORMERS_OFFLINE=1. These can only be set before CUDA initialises.
42.	Blackwell tensor core optimisation. TF32 matmul and cuDNN benchmark are enabled, with float32_matmul_precision="high".
43.	Forced offline mode. No model downloads at runtime. If a model is missing, the system errors loudly rather than silently fetching from HuggingFace.
The lessons that produced these defenses are documented in feedback_wsl_gpu_stability.md and were paid for in many lost hours of training time during the six-day sprint.
7.5 The Constellation Subsystem
The constellation subsystem (src/phoenix/video/) is the heart of Phoenix. It contains four files:
•	`expression_skills.py` — the four-level expression hierarchy (§5).
•	`constellation_controller.py` — the runtime expression controller (§5.5).
•	`constellation_guard.py` — the runtime identity guard (§6).
•	`gpu_guard.py` — the GPU safety harness (§7.4).
Together these four files implement the full Teleological Constellation System: identity definition (the centroids loaded by the guard), behavioral vocabulary (the expression skills extracted from FLAME data), runtime control (the controller), and runtime validation (the guard). The four MCP tools that expose this subsystem (clipcannon_list_constellations, clipcannon_list_skills, clipcannon_clone_video, clipcannon_expression_sequence) live in src/clipcannon/tools/constellation.py and are registered through the same dispatcher as every other ClipCannon tool.

8. Part VI — The Clone Video Pipeline
8.1 End-to-End Architecture
The clone video pipeline ties everything together:
Source video (16 minutes of subject)
    │
    ▼
ClipCannon ingest → analysis.db
    │
    ▼
Embedding extraction → all_embeddings.npz (7 modalities)
    │
    ▼
Constellation construction → frozen centroids
    │
    ▼
Curated training data preparation (mouth-bbox-tagged clips)
    │
    ▼
LoRA training of EchoMimicV3 (mouth-weighted loss + corrected flow sign)
    │
    ▼
At inference time:
    ConstellationController → per-frame NL prompts
    Audio (synthesized from cloned voice via Qwen3-TTS)
    Reference image (face-centered crop from source)
        │
        ▼
    EchoMimicV3 + Santa LoRA (1.86B DiT) → generated video
        │
        ▼
    ConstellationGuard → frame-by-frame validation
        │
        ▼
    Output video (with audio remux)

Every stage is governed by the constellation. Embedding extraction populates it. Training pulls toward it. Inference conditioning uses it. Runtime validation enforces it.
8.2 Data Preparation
The data preparation pipeline lives in scripts/curate_training_data.py, scripts/precompute_face_landmarks.py, and scripts/label_training_clips.py. The current production pipeline (after the six-day sprint) produces 2 362 training clips with the following structure for each clip:
•	A 1.96-second video segment at 25 fps (49 frames), encoded as a 13-frame temporal latent through the EchoMimicV3 VAE.
•	A reference latent (single frame).
•	Audio embeddings from Wav2Vec2 cross-attention.
•	CLIP features of the reference frame.
•	Text context (the transcript words spoken in the clip plus the constellation tag).
•	A mouth mask precomputed from YOLOv5-Face landmarks.
•	A viseme sequence derived from the CMU pronunciation dictionary (15 MPEG-4 visemes) with 7 916 real samples and 100% viseme coverage.
•	Metadata including emotion tag, speaking energy, viseme presence, and curation category.
The curation logic uses ClipCannon's analysis tables: the transcript_words table provides word-level timing, prosody_segments provides energy and speaking rate, emotion_curve provides emotion tagging, scenes provides scene boundaries, and speaker diarization filters out the interviewer. Each clip is tagged with speaking vs. breathing, an emotion label, energy level, and the visemes present.
8.3 LoRA Training on EchoMimicV3
The training script scripts/train_santa_lipsync.py fine-tunes EchoMimicV3 (a 1.86B DiT based on Wan2.1-Fun) with LoRA rank 256, alpha 512, on the eight target modules: q, k, v, o, k_audio, v_audio, k_img, v_img. The original target list included q_audio, but a code audit revealed that q_audio(q) output is overwritten immediately by upstream code (a q_auido typo at line 843 of the EchoMimicV3 transformer means the q_audio path never reaches the loss), so q_audio is excluded. Training runs for 3 000 steps at learning rate 1e-4 with cosine decay, expecting ~90–120 minutes on the RTX 5090 with VRAM peaking around 14 GB.
The flow-matching loss is mouth-weighted: the gradient is scaled 10× in the mouth region, 2× in the face region, and 1× in the background. This converts the mouth from ~2% of the latent (where it would otherwise contribute negligibly to the loss) into ~19% of the effective gradient signal — large enough that the model has a real incentive to get the mouth right rather than nailing the face/beard/background and ignoring the mouth.
Sampling uses WeightedRandomSampler with viseme-rarity weights, oversampling rare visemes (OY, AW, UH) by 2–4× to ensure coverage of all 15 MPEG-4 visemes during training rather than just the common ones.
8.4 Mouth-Weighted Loss and the Flow-Matching Sign Bug
The most important discovery in the entire six-day sprint was a sign bug in the flow-matching loss that affected every previous LoRA iteration (v1 through v5). The original code computed:
target = latent - noise   # WRONG

But the EchoMimicV3 scheduler fm_solvers.py uses a velocity convention where:
target = noise - latent   # CORRECT

This means every previous training run was teaching the model to add noise to clean video rather than remove it. The model was learning to anti-denoise. The only reason any previous iteration produced anything recognisable was because the inpainting condition passed the reference frame through unchanged at inference time, masking the broken denoising. Lip sync never had a chance.
This was discovered by a forensic Sherlock investigation that compared the loss formulation against the scheduler's velocity definition algebraically. It is documented in docs/inspect/08_training_audit.md along with five other blockers:
44.	Wrong flow-matching sign (above).
45.	cv2 video read returned native 60 fps instead of 25 fps, making clips 2.4× shorter than the audio.
46.	Mouth bounding boxes computed from geometric ratios on scene_map bboxes landed on Santa's nose rather than his mouth. Replaced with YOLOv5-Face 5-point landmarks giving actual mouth corners.
47.	q_audio LoRA target was dead code (above).
48.	22 GB of audio features accumulated in RAM during preprocessing. Streamed to disk per-clip instead.
49.	The viseme curriculum was advertised as weighted but actually used uniform sampling. Replaced with WeightedRandomSampler.
Each of these was verified individually with evidence (unit test, file inspection, cross-reference to the scheduler code) before the training pipeline was rebuilt. The current state of the pipeline has all six fixes applied, all 2 362 clips encoded, and is ready to train.
8.5 Inference and Multi-Chunk Generation
At inference time the generation flow is:
1. ConstellationController.set_emotional_state("warm_conversational", 0.8)
2. ConstellationController.get_prompt_sequence(duration_s=10.0, fps=25)
   → 250 (frame_idx, prompt) tuples
3. EchoMimicV3 inference:
   - Reference image (Santa portrait, 768×768 face-centered crop)
   - Audio (10 seconds, generated by Qwen3-TTS in cloned voice)
   - LoRA path: ~/.clipcannon/models/santa/echov3_lipsync/final/
   - Text prompt: per-frame prompt from controller
   - Sampler: Flow_DPM++
   - Steps: 20
4. ConstellationGuard.validate_batch(generated_frames)
   → If any frame fails, regenerate the chunk
5. Audio remux (no CodeFormer — explicit rule from feedback_lipsync_quality.md)
6. Output: clean clone video with cosine similarity ≥ 0.70 visual to centroid

For videos longer than the model's native 10-second window, the system uses multi-chunk generation: 30-second videos are produced from 10-second chunks with 0.5-second audio overlap and a pixel crossfade between chunks. Each chunk uses the original reference image rather than the previous chunk's last frame to prevent quality degradation chains.
The output resolution is 768×768 (the model's native resolution, finally used after the SITREPs documented that earlier runs accidentally generated at 480×480 or 576×384 because the reference image was too small and get_sample_size() scaled the output to match).

9. Part VII — VoiceAgent and the Real-Time Meeting Bot
9.1 Architecture
VoiceAgent (src/voiceagent/) is the real-time conversational layer. Architecturally:
Audio in (mic / meeting)
    │
    ▼
Wake Listener ("Hey Jarvis") — always-on
    │
    ▼
Voice Activity Detection → Streaming ASR (Whisper) → Semantic Endpointing
    │
    ▼
Brain (Qwen3-8B LLM)
    ├── Context Manager
    ├── Prompt Templates
    └── Response generation
    │
    ▼
Streaming TTS (faster-qwen3-tts 0.6B)
    │
    ▼
Audio out (local / WebSocket / meeting voice)

Wake-word activation uses Hey Jarvis and is always-on; this is a hard rule recorded in feedback_wake_listener_always_on.md. Voice command detection in audio/voice_command_detector.py recognizes commands like "stop", "repeat", and "louder". Acoustic echo cancellation in audio/aec_filter.py prevents the agent from hearing its own voice. Semantic turn end pointing in asr/endpointing.py detects when the speaker has finished a thought rather than just paused.
9.2 Latency Budget
Component	Target	Notes	
ASR (Whisper turbo)	<500 ms	Streaming chunks	
VAD	300 ms	Silero VAD	
LLM (Qwen3-8B)	<1 s	First token	
TTS (0.6B model)	~500 ms	Time-to-first-byte, streaming	
End-to-end speech-to-speech	<2 s	Target	

The TTS model is faster-qwen3-tts 0.6B, used specifically because of its ~500 ms TTFB. ClipCannon itself uses the larger 1.7B model for non-real-time voice cloning where SECS ≥ 0.95 is the priority. The two-tier architecture is documented in project_voiceagent_tts.md.
9.3 The Meeting Bot Subsystem
The meeting bot (src/voiceagent/meeting/, 25 files) is a full-featured agent that joins Zoom and browser meetings (Google Meet, Teams) as an AI clone. The architectural choice that finally worked, after several alternatives failed, is Xvfb + Playwright: Chrome runs headful on virtual display :99, invisible to the user, with a separate browser session that has its own Google login. This is the same architecture as commercial bots like tl;dv, Recall.ai, and Otter.ai.
Video delivery uses Insertable Streams. Chrome's --use-fake-device-for-media-stream creates a track with real camera metadata. MediaStreamTrackProcessor reads the fake device frames; we replace the pixel content with our rendered frames; MediaStreamTrackGenerator outputs with spoofed metadata. Meet sees proper getSettings()/getCapabilities() and accepts the track. This was the sixth video delivery approach tried — canvas captureStream, Y4M FIFO, v4l2loopback, raw MediaStreamTrackGenerator, and others all failed. Insertable Streams is documented as the only path that works in feedback_v4l2loopback.md.
Audio delivery uses JavaScript WebAudio API injection rather than PulseAudio routing. Chrome's fake device bypasses PulseAudio, so audio must be injected via JS to be heard. This is documented in feedback_audio_delivery.md.
A Chrome extension (installed on the user's Windows Chrome) detects Google Meet URLs and POSTs to localhost:9877, where the bot server (bot_server.py) launches the bot via Playwright on Xvfb. The extension is a thin trigger only — it does no recording or injection.
The 25 meeting bot files cover:
•	Bot infrastructure — bot_join.py, bot_server.py, zoom_bot.py, browser_bot.py, app_controller.py.
•	Audio/video — audio_capture.py, voice_output.py, webcam_writer.py.
•	AI — reasoning_controller.py, meeting_behavior.py, avatar_rt.py (Phoenix integration), idle_renderer.py, responder.py.
•	Transcription — transcriber.py, transcript_format.py, transcript_store.py, meeting_summary.py.
•	Realism — devices.py, address_detector.py, realism.py.
•	Integration — mcp_client.py for calling ClipCannon tools from inside the meeting context.
9.4 Reasoning Controller and AvatarCommand
The reasoning_controller.py is a three-tier always-on reasoning architecture documented in project_reasoning_controller.md. It runs in 800 ms cycles, observes the meeting transcript, classifies intent (IDLE, LISTEN, RESPOND, etc.), and produces an AvatarCommand containing blendshapes, gaze, pose, and prosody for the next time step. This command is the bridge between Gemma 4's high-level reasoning ("the avatar should be empathetic right now") and Phoenix's low-level rendering ("set constellation to empathetic_concern at intensity 0.85").
In the meeting context, the controller maps the conversation state to a behavioural constellation, the constellation controller generates per-frame prompts, the LoRA-trained EchoMimicV3 model produces video frames, and the constellation guard validates them. The full chain runs at 25 fps with end-to-end latency under 2 seconds for the speech path and under 100 ms for the video path.

10. Part VIII — Six Days, Seventeen Approaches
10.1 Why a Negative-Result Section Belongs in a White Paper
A white paper that omits its failures gives a misleading picture of the design space. The current architecture is the result of seventeen different approaches to the clone video problem, sixteen of which failed for substantive technical reasons. Reading those failures is the fastest way to understand why the surviving architecture looks the way it does. This section documents them in chronological order.
10.2 The Approaches, In Order
Day	Approach	Outcome	
Apr 5	Blendshape + FaceWarper	Produced a still photo with imperceptible mouth shifts. "Doing literally nothing." Deleted 10 385 lines.	
Apr 5	Gaussian Splatting v1 (5 K iterations, tex_size 128)	Output was a blurry blob.	
Apr 5	Gaussian Splatting v2 (target 200 K, LPIPS, 58 K Gaussians, 512×512)	Training crashed at step 71 K. 50 K checkpoint was a blurry pink blob. LPIPS plateaued at 0.43 when <0.2 was needed.	
Apr 5–6	Semantic Transformer + FaceWarper	Worked for face quality but only the mouth animated; rest of face frozen. Static and uncanny.	
Apr 7	CogVideoX-2b LoRA (rank 256, 3 000 steps)	8 fps native. Eye glitches. Identity locked but temporal quality insufficient.	
Apr 7	CogVideoX-5b-I2V LoRA (rank 128, 3 000 steps)	Better than 2b but still 8 fps with eye artefacts.	
Apr 7	Hallo3 on RTX 5090	14.55 B params. BF16 weights = 29.1 GB. 650 s/step. Not viable on 32 GB. Deleted 49 GB of weights.	
Apr 7	EchoMimicV3 base model	Excellent 1-second clip ("user genuinely liked it"). 15-second version degraded badly after 2 s.	
Apr 7	EchoMimicV3 LoRA v1 (194 random clips, rank 256)	Sharp identity, generic open/close lip sync.	
Apr 7	EchoMimicV3 LoRA v2 (190 curated clips, rank 256)	Best face quality. Breathing-aware. Generic lip sync.	
Apr 7	EchoMimicV3 LoRA v3 (curated + viseme prompts)	Same as v2. Viseme prompts ignored by audio attention.	
Apr 8	EchoMimicV3 LoRA v4 (temporal video clips, 13 latent frames, black bars)	Loss converged but output blurry and distorted.	
Apr 8	EchoMimicV3 LoRA v5 (temporal, face-cropped)	Loss 0.9494. Still blurry. Concluded temporal video training is a dead end for this architecture.	
Apr 8	MuseTalk 1.5 post-processing	Still has blur artefacts. Lip sync bad. The "no blur" promise didn't deliver.	
Apr 8	LatentSync + InsightFace	Best lip sync the user had seen, BUT InsightFace fails on bearded faces (score 0.3–0.5 vs needed 0.9).	
Apr 9	LatentSync + YOLOv5-Face landmarks	Detection fixed. Affine alignment still displaced lips when Santa moved his head.	
Apr 9	HunyuanVideo-Avatar (14.55 B params)	2+ hours per 5 s with cpu_offload. DeepCache disables audio adapters by default. Smart fix implemented but model still too slow. Bad output quality.	
Apr 9–10	Mouth-weighted LoRA training (the current pipeline)	Six Sherlock blockers identified and fixed. Encoded. Ready to train.	
Apr 10	Hallo3 on H100 via Vast.ai	Docker build failed (pyav 14.0.1 dep conflict). Backup plan if local training fails.	

Seventeen rows, sixteen of them failures. The user described being in the seventeenth attempt as "the first training that has a real shot at phoneme-accurate lip sync".
10.3 Cross-Cutting Discoveries
Across the seventeen approaches several discoveries kept appearing. They are now design rules embedded in the system:
50.	Insertable Streams is the only video delivery path that works for Google Meet. Browsers reject everything else.
51.	Xvfb + Playwright is the production architecture for meeting bots. It is what every commercial bot uses.
52.	PulseAudio routing causes Whisper hallucination. The bot's Chrome must be on a dedicated null sink.
53.	WSL2 CUDA driver is unstable under sustained GPU load. Concurrent CUDA contexts crash the driver. Operations must be serialized. cu12 packages must be removed (mixing cu12 and cu13 causes cuSOLVER crashes).
54.	InsightFace fails on bearded faces. Detection scores collapse. YOLOv5-Face works.
55.	Flow-matching loss is sign-sensitive. Wrong sign means the model learns to anti-denoise. Verify against the scheduler algebraically.
56.	Mouth is only 2% of the latent. Uniform loss gives no incentive for lip accuracy. Mouth-weighted loss is essential.
57.	Source video frame rate matters. EchoMimicV3 source was 60 fps native, cv2 read at native rate, clips were 2.4× shorter than audio. Use ffmpeg with explicit -r 25.
58.	Mouth bbox must come from real landmarks. Geometric ratios put the mouth on the nose.
59.	Reference image resolution determines output resolution. EchoMimicV3 get_sample_size() scales to match the reference.
60.	Single-frame training preserves identity better than temporal video training for this architecture. Temporal training averaged across frames, producing temporally consistent blur.
61.	Models must never be downloaded at runtime. HF_HUB_OFFLINE=1 and TRANSFORMERS_OFFLINE=1. Pre-cache and error if missing.
62.	No CodeFormer on lip-sync output. It was specifically banned in feedback_lipsync_quality.md because it degraded the result.
63.	VAE blur is architectural in LatentSync. Cannot be fixed by swapping the VAE; that destroys the output.
64.	MouthMemory and ER-NeRF are not viable alternatives — both were tried and failed.
65.	The constellation framing came from frustration. After enough drift, the user demanded a definition of "still being Santa" that the system could check at every step. That definition became the constellation.
10.4 What Survived
After all the failures, the surviving architecture is:
•	ClipCannon as the foundation pipeline (unchanged from the start).
•	The Teleological Constellation System as the alignment mechanism, derived from the failures.
•	Phoenix as the GPU-native render engine, with the FaceWarper, Gaussian Splatting renderer, NVENC encoder, and Perceiver clone model removed; only FLAME, lip-sync, physics-face, compositing, and effects retained.
•	EchoMimicV3 + Santa LoRA as the primary clone video generator, with corrected flow-matching sign and mouth-weighted loss.
•	PhysicsFaceEngine as a deterministic baseline and fallback for fast preview.
•	VoiceAgent + meeting bot as the real-time deployment surface.
•	Hallo3 on H100 via Vast.ai as a backup plan if the local LoRA training does not produce phoneme-accurate lip sync.
That is the system as it stands today.

11. Part IX — Theoretical Framing
11.1 Alignment as Geometric Constraint
The standard framing of AI alignment is scalar reward maximisation: we train a model to maximize a reward signal that approximates human preferences (RLHF, DPO, Constitutional AI). The model is "aligned" when it produces outputs that score highly under the reward.
The constellation framing is geometric constraint: we define alignment as proximity to a frozen point in a multi-dimensional embedding space, and constrain the model to remain near that point at every level — architecture, training loss, runtime validation.
The two framings have different properties:
Property	Scalar reward	Geometric constraint	
Target	Reward signal (proxy for preference)	Frozen centroid (direct definition of identity)	
Drift	Possible (reward hacking, distribution shift)	Bounded by threshold	
Verifiability	Indirect (must measure preference)	Direct (cosine similarity)	
Human feedback	Required ongoingly	Required only for centroid construction	
Failure mode	Goodharting, reward hacking	Frame rejection, regeneration	
Generalization	Implicit (hopeful)	Explicit (the centroid is what generalizes)	

The geometric constraint approach is not a replacement for scalar reward in every situation. It works best for problems where the alignment target is definable — where you can write down what success looks like in vector form. Identity-locked avatar generation is the canonical example. Safety properties of language models that can be embedded (e.g., "matches the safety profile of these reference outputs") may be another.
11.2 Verifiability Without Human Feedback
The constellation guard provides a guarantee that no scalar reward system can match: every output is verified, individually, against a fixed criterion, before being released. There is no batch evaluation, no hold-out set, no offline measurement — just a per-output cosine similarity check that runs at inference time.
The verification does not require a human in the loop. The centroids were built once, from reference data, by a deterministic procedure. The threshold is configurable but invariant during operation. The check is fast (a single forward pass through SigLIP plus a cosine similarity), runs on the same hardware as the model, and produces a boolean result with a human-readable rejection reason.
This is the property that makes the system suitable for autonomous deployment. A meeting bot that must run for hours without supervision needs a reliable test that the avatar is still itself. The constellation guard provides exactly that.
11.3 Generalisation Beyond Avatars
The principle generalizes in several directions.
Other generative video tasks. Any generative video task with a defined target can use the constellation framing. Style transfer (the centroid is the style), animation cloning (the centroid is the animator's vocabulary), product visualization (the centroid is the product's appearance manifold), and broadcast-safe content generation (the centroid is the safety profile) all fit naturally.
Other modalities. Constellations can be built for any modality with a frozen embedding model. Text constellations using sentence transformers; audio constellations using WavLM; pose constellations using OpenPose; gesture constellations using HaMeR. The constraint is that the embedder must be frozen (so the centroid does not drift) and shared between centroid construction and runtime validation (so the comparison is meaningful).
Multi-agent scenarios. A team of AI agents could each have their own constellation, defining their respective identities. A coordinator could route messages to the agent whose constellation best matches the request, providing a form of dispatch that does not require labelled training data.
Safety auditing. A "safe behavior" constellation could be built from examples of acceptable model behavior. Any output that falls outside the constellation could be flagged for review. This is a different shape of safety than rejection sampling against a classifier, because the constellation is a positive definition of acceptable behavior rather than a negative definition of unacceptable behavior.
11.4 Limitations and Open Problems
The constellation framing has real limitations:
66.	It requires reference data. You cannot align a model to a target you do not have examples of. For an identity that has never been recorded, there is no constellation to build.
67.	Centroid construction is sensitive to data quality. A constellation built from low-quality or biased reference data will inherit the bias. Filtering and curation are essential.
68.	The threshold is empirical. There is no first-principles way to choose the cosine similarity threshold; it must be tuned per modality and per deployment.
69.	Generalization within the manifold is implicit. The constellation defines the centre but not the radius of acceptable variation. The model can drift within the manifold without being rejected, and the user must verify that intra-manifold variation is acceptable.
70.	It does not solve general-purpose alignment. Aligning a chatbot to "be helpful and harmless" is not obviously a constellation problem. The framing works for problems with definable identity-like targets, not for problems where the target is a general capability or value.
71.	Runtime cost is non-trivial. Re-embedding every frame through SigLIP costs ~10 ms per frame on the RTX 5090. For 25 fps video this is 250 ms per second, a 25% overhead on the rendering budget. Faster encoders or batched validation can reduce this, but it is not free.
Despite these limitations, for the specific class of identity-aligned generation problems the framing addresses, it is a substantial improvement over scalar-reward approaches. It provides per-output guarantees, requires no ongoing human feedback after centroid construction, and has a clean theoretical interpretation in terms of geometry rather than statistics.

12. Part X — System Inventory
12.1 Source Code Layout
src/
├── clipcannon/                       # 247 .py files, ~67 585 LOC
│   ├── __init__.py                   # Package root, __version__
│   ├── server.py                     # MCP server entry point
│   ├── config.py                     # Pydantic config + dot-notation access
│   ├── exceptions.py                 # ClipCannonError hierarchy
│   ├── db/                           # SQLite + sqlite-vec layer
│   ├── gpu/                          # ModelManager, precision detection
│   ├── provenance/                   # Hash chain, recorder
│   ├── billing/                      # HMAC integrity, license client
│   ├── editing/                      # EDL, captions, smart crop, motion, overlays
│   ├── rendering/                    # FFmpeg pipeline, profiles, batch, cache
│   ├── audio/                        # Music, MIDI, SFX, mixing, cleanup
│   ├── voice/                        # Qwen3-TTS, verification, profiles
│   ├── avatar/                       # LatentSync lip-sync engine
│   ├── tools/                        # 14 modules implementing 58 MCP tools
│   ├── pipeline/                     # 23-stage DAG and stage implementations
│   └── dashboard/                    # FastAPI dashboard on port 3200
├── license_server/                   # FastAPI billing server on port 3100
├── phoenix/                          # GPU-native avatar engine
│   ├── adapters/                     # Audio2Face, blendshape types
│   ├── behavior/                     # Cross-modal detector, gesture selector
│   ├── expression/                   # Eye behaviour, gesture library
│   ├── render/                       # FLAME, lip-sync, physics-face, compositor
│   └── video/                        # Constellation system (the heart)
│       ├── expression_skills.py      # 4-level hierarchy
│       ├── constellation_controller.py
│       ├── constellation_guard.py
│       └── gpu_guard.py
└── voiceagent/                       # Real-time agent + meeting bot
    ├── activation/                   # Wake word, hotkey
    ├── asr/                          # Whisper, VAD, endpointing
    ├── audio/                        # AEC, semantic turn detection
    ├── brain/                        # Qwen3-8B LLM
    ├── conversation/                 # State machine, manager
    ├── tts/                          # faster-qwen3-tts streaming
    ├── transport/                    # Local audio, WebSocket
    ├── adapters/                     # ClipCannon adapter, fast TTS
    └── meeting/                      # 25-file meeting bot subsystem

12.2 Dependencies
Core: mcp ≥ 1.0, pydantic ≥ 2.0, httpx ≥ 0.25, fastapi ≥ 0.110, uvicorn ≥ 0.30, python-jose ≥ 3.3, Pillow ≥ 10, numpy ≥ 1.26, scipy ≥ 1.12, stripe ≥ 8.0, sqlite-vec ≥ 0.1.
ML ([ml] extra): torch ≥ 2.3, faster-whisper ≥ 1.0, transformers ≥ 4.40, sentence-transformers ≥ 3.0, demucs ≥ 4.0, librosa ≥ 0.10.
Phase 2 ([phase2] extra): pydub, pedalboard, midiutil, pyfluidsynth, ace-step, audiocraft, mediapipe, opencv-python, rembg.
Phase 3 ([phase3] extra): Qwen3-TTS, marksverdhei/Qwen3-Voice-Embedding-12Hz-1.7B, resemble-enhance, LatentSync 1.6.
External (Phoenix-specific, not in pip dependencies): EchoMimicV3 1.86 B at ~/echomimic_v3/, Wan2.1-Fun base model, chinese-wav2vec2-base, FLAME model files.
12.3 Trained Models on Disk
Model	Path	Size	Status	
EchoMimicV3 base weights	~/echomimic_v3/pretrained_weights/	~26 GB	Active	
Santa LoRA (lipsync, mouth-weighted)	~/.clipcannon/models/santa/echov3_lipsync/final/	~1.2 GB	Pipeline ready, training pending	
7-modality embeddings (constellation)	~/.clipcannon/models/santa/embeddings/all_embeddings.npz	8.8 MB	Active	
FLAME parameters (Santa)	~/.clipcannon/models/santa/flame_params.npz	1.3 MB	Active (3 477 frames)	
Face landmarks (YOLOv5-Face, 25 fps)	~/.clipcannon/models/santa/face_landmarks_25fps.npz	2 MB	Active (24 397 frames, 90% detected)	
Reference image (Santa, 768×768)	~/.clipcannon/models/santa/reference/santa_fullframe_600s.png	~3 MB	Active	
Voice profile (boris)	~/.clipcannon/voice_profiles.db	-	Active (SECS ≥ 0.95)	
Teleological conditioning (training data)	~/echomimic_v3/datasets/santa_teleological/conditioning/	30 GB	2 362 pre-encoded clips	

12.4 Test Coverage
Pytest collects 1 131 tests across 85 files. The full FSV (Full State Verification) suite consists of ten scripts; the four primary scripts run 750 individual checks. The constellation system specifically has 22 guard tests (covering modality validation, threshold enforcement, batch validation, edge cases), 30 controller tests (state, queuing, blending, transitions), and 31 expression skill tests (FLAME extraction, K-Means clustering, skill registration, prompt generation). The viseme system has 34 tests covering CMU dictionary lookups, MPEG-4 viseme mapping, and per-frame conditioning interpolation.
12.5 Forensic Reports
Ten Sherlock forensic investigation reports document the discovery of the bugs that broke previous training runs:
•	docs/inspect/01_data_pipeline.md — HunyuanVideo-Avatar data issues
•	docs/inspect/02_model_architecture.md — DeepCache and guidance scale audit
•	docs/inspect/03_face_detection.md — Face mask coverage analysis
•	docs/inspect/04_output_quality.md — Frame-by-frame quality metrics
•	docs/inspect/05_correct_usage.md — How HunyuanVideo should be configured
•	docs/inspect/06_smart_deepcache_audit.md — Smart DeepCache code review
•	docs/inspect/07_labeling_audit.md — Labelling pipeline audit (5 bugs found)
•	docs/inspect/08_training_audit.md — Training script audit (6 blockers found)
•	docs/inspect/09_data_pipeline_audit.md — Data alignment audit (60 fps bug)
•	docs/inspect/10_fix_verification.md — Final verification of all blockers
These reports follow the Sherlock methodology: assume all code is guilty until proven innocent through forensic evidence, with every claim backed by a reproducible test, file inspection, or cross-reference to authoritative source code.

13. Conclusion
ClipCannon and the Teleological Constellation System were built together over a few intensely concentrated months of development, including the six-day sprint documented in twelve session SITREPs. The foundation pipeline exists because the avatar problem demanded extraordinary infrastructure: video understanding to extract a person's behavioral vocabulary, a provenance system to trust the extracted data, an editing engine to compose training material, a voice cloning engine to synthesize the audio side of the clone, and a real-time agent layer to deploy the result. The constellation system exists because, after enough failed approaches to the avatar problem, the missing ingredient turned out to be a definition of what success meant — a frozen target the system could orbit at every step of training and inference.
The architecture has three nested layers:
72.	ClipCannon is the foundation. It understands video, edits video, renders video, generates audio, clones voices, and exposes everything through 58 MCP tools backed by a provenance chain and an HMAC-secured billing system. Its 247 source files and 1 131 tests are the substrate on which the avatar system is built.
73.	The Teleological Constellation is the alignment mechanism. Seven embedding modalities define a 4 044-dimensional identity manifold. A four-level expression hierarchy (Action Units → Groups → Skills → Constellations) translates emotional intent into per-frame conditioning. A runtime guard re-embeds every generated frame and rejects any that drift outside the manifold. The whole system is teleological in the strict sense: every component is organized around the same end-state.
74.	The clone video pipeline is the first concrete application. EchoMimicV3 fine-tuned via LoRA with mouth-weighted loss and a corrected flow-matching sign, conditioned by the constellation controller and validated by the constellation guard, produces video of a specific person with controllable expressions across eight emotional states while maintaining identity consistency at cosine similarity ≥ 0.70 (visual) and ≥ 0.80 (speaker).
The deeper claim of this paper is that alignment can be defined as proximity to a multi-dimensional identity manifold, that this definition is verifiable without ongoing human feedback, and that the resulting system has properties that scalar-reward approaches cannot match. The claim is demonstrated on a single subject ("Santa") trained from sixteen minutes of interview video, and the architecture is general enough that it can be applied to any generative model with a frozen embedding-based perception layer.
The system is incomplete in important ways. The lip-sync LoRA is encoded but not yet trained at the time of writing (the seventeenth attempt is queued). The multi-clone deployment is unbuilt. The H100 backup plan via Vast.ai is blocked on a Docker dependency conflict. Several modalities of the constellation are populated with fewer samples than would be ideal. The runtime overhead of constellation validation is non-trivial. None of these undermine the architecture; they are the next set of problems to solve.
What does survive is the conviction that the framing is right. After seventeen approaches and six sleepless days, the engineering history converged on a single answer: define what you want as a frozen point in embedding space, organize everything around that point, and the system finally has a chance of producing what you asked for. The constellation is the telos, and everything orbits it.

Appendix A — Glossary
Term	Definition	
Action Unit (AU)	A FACS-defined unit of facial muscle movement (Ekman & Friesen, 1978). 46 in total; 17 used here.	
Behavioural constellation	A named emotional state expressed as a cycling sequence of expression skills.	
Constellation centroid	The L2-normalised mean of all embeddings of a target identity in a given modality.	
Constellation Guard	The runtime validator that re-embeds generated frames and rejects those outside the centroid neighborhood.	
EchoMimicV3	A 1.86 B-parameter audio-driven talking-head video diffusion model based on Wan2.1-Fun.	
EDL	Edit Decision List — declarative video editing data model.	
Expression skill	A named behavioural expression with onset/peak/offset phases and a natural-language prompt.	
FACS	Facial Action Coding System — Ekman's framework for describing facial muscle movements.	
FLAME	Faces Learned with an Articulated Model and Expressions — parametric head model with 100 expression coefficients.	
Flow matching	Diffusion-model training method where the network predicts the velocity field rather than noise.	
HMAC integrity	Cryptographic balance signing using a machine-derived key to prevent tampering.	
LoRA	Low-Rank Adaptation — parameter-efficient fine-tuning by adding trainable low-rank matrices.	
Micro-expression group	An atomic group of Action Units that fire together, discovered by clustering FLAME coefficients.	
Mouth-weighted loss	Flow-matching loss with 10× gradient in the mouth region, 2× face, 1× background.	
Provenance chain	Tamper-evident hash chain linking every pipeline operation to its predecessor.	
SigLIP	Sigmoid Loss Image-Text Pretraining — vision encoder used for visual modality embedding.	
TCT	Teleological Constellation Training — the alignment method described in this paper.	
Teleological	Oriented toward purpose or end-state.	
Viseme	A visual unit of speech corresponding to a phoneme. 15 in MPEG-4.	
VRAM budget	Hard cap on GPU memory usage enforced by gpu_guard.check_vram.	

Appendix B — Constellation Definitions
The eight production constellations and their skill sequences:
warm_conversational    → warm_smile, gentle_nod, eye_contact, slight_gesture
emotional_recall       → thoughtful_pause, gaze_down, voice_waver, brave_smile, deep_breath
happy_storytelling     → genuine_laugh, eyes_wide, animated_gesture, warm_smile, emphatic_nod
empathetic_concern     → brow_furrow, soft_eyes, slow_nod, lip_press, head_tilt
thoughtful_reflection  → gaze_up, slight_frown, pause, slow_blink, chin_rest
excited_sharing        → eyes_wide, rapid_speech, lean_forward, hand_gesture, genuine_laugh
solemn_gravity         → steady_gaze, slow_speech, minimal_movement, slight_nod, deep_breath
gentle_humor           → eye_twinkle, half_smile, slight_pause, warm_chuckle

The 34 expression skill prompts are stored in SKILL_PROMPTS in src/phoenix/video/expression_skills.py. Each prompt is a natural-language fragment injected into the EchoMimicV3 generation prompt for the duration of the skill.
Appendix C — Pipeline Stage Reference
The 23 pipeline stages and their dependencies (see §3.3 for the full DAG):
#	Stage	Required	Depends on	
1	probe	Yes	(none)	
2	vfr_normalize	Yes	probe	
3	audio_extract	Yes	vfr_normalize	
4	frame_extract	Yes	vfr_normalize	
5	source_separation	No	audio_extract	
6	visual_embed	No	frame_extract	
7	ocr	No	frame_extract	
8	quality	No	frame_extract	
9	shot_type	No	frame_extract, visual_embed	
10	storyboard	No	frame_extract	
11	transcribe	Yes	audio_extract	
12	acoustic	No	audio_extract	
13	scene_analysis	No	frame_extract, transcribe	
14	semantic_embed	No	transcribe	
15	narrative_llm	No	transcribe	
16	speaker_embed	No	audio_extract, transcribe	
17	emotion_embed	No	audio_extract	
18	reactions	No	audio_extract	
19	profanity	No	transcribe	
20	prosody_analysis	No	source_separation, transcribe	
21	chronemic	No	transcribe, speaker_embed	
22	highlights	No	emotion_embed, reactions, semantic_embed, visual_embed, quality, speaker_embed, chronemic	
23	finalize	Yes	(all prior stages)	

Each stage produces one or more provenance records and writes to its dedicated tables. The 16 stages tracked in stream_status are: source_separation, visual, ocr, quality, shot_type, transcription, semantic, emotion, speaker, reactions, acoustic, beats, chronemic, storyboards, profanity, highlights. The scene_analysis, narrative_llm, and prosody_analysis stages create their own dedicated tables (scene_map, narrative_analysis, prosody_segments) on demand.

End of paper.

OCR Provenance
A New Paradigm for SaaS in the Age of Local AI
 
White Paper | March 2026
Author: Chris Royse
Affiliation: Kansas State University, Graduate Student, Mass Communication Contact: arcibu@ksu.edu
System Version: OCR Provenance MCP Server v1.3.6 153 MCP Tools 
Table of Contents
1.	The Problem with Traditional SaaS
2.	The OCR Provenance Model: Local-First, Commercially Viable
3.	System Architecture
4.	The Billing Revolution: Zero-Overhead Monetization
5.	Security Architecture: Air-Gapped by Default
6.	The Document Intelligence Platform
7.	GPU Processing: Leveraging End-User Inference
8.	Cloud GPU Mode: Bridging the Hardware Gap
9.	Provenance and Compliance: Enterprise-Grade Trust
10.	Scaling Economics: Why This Model Wins
11.	The MCP Protocol: AI-Native Software Distribution
12.	Case Study: Legal Document Intelligence
13.	Future Implications
14.	Conclusion 
Abstract
The Software-as-a-Service model has remained structurally unchanged for two decades: vendors host software on their infrastructure, users pay subscriptions, and all data flows through the vendor's servers. This model carries inherent costs for the vendor (compute, storage, bandwidth, compliance liability) and inherent risks for the user (data exposure, vendor lock-in, latency, regulatory barriers). OCR Provenance MCP Server introduces a fundamentally different commercial architecture -- one where the software runs entirely on the user's own hardware, processes all data locally with zero cloud dependency, yet still operates as a metered, commercially viable product. This white paper describes the system in full: its technical architecture, its novel billing model, its security posture, and why this approach represents a paradigm shift for how AI-powered SaaS products can be built, monetized, and scaled -- with zero operational overhead for the vendor.
 
1.	The Problem with Traditional SaaS
Traditional SaaS operates on a straightforward bargain: the vendor hosts the software, the user pays for access. This model has dominated for two decades because it solved real problems -- no installation, automatic updates, accessibility from anywhere. But as AI workloads have grown more compute-intensive and data has become more sensitive, the traditional model has developed critical fractures.
1.1	The Vendor's Cost Problem
Every traditional SaaS company running AI workloads faces the same arithmetic: GPU compute is expensive. A single NVIDIA H100 costs approximately $30,000 and depreciates rapidly. Cloud GPU instances on AWS or GCP cost $2-8 per hour. For a document processing platform handling thousands of pages per day per customer, the compute costs alone can consume 40-70% of revenue. Add storage, bandwidth, compliance infrastructure, and support staff, and the vendor's margin shrinks rapidly.
This creates a perverse incentive: vendors must either charge high prices (limiting adoption), cut processing quality (reducing value), or operate at a loss while seeking scale (burning capital). None of these paths is sustainable for an independent developer or small team.
1.2	The User's Trust Problem
Users processing sensitive documents -- legal contracts, medical records, financial statements, government filings -face an irreconcilable tension. They need AI-powered processing, but they cannot risk their data leaving their infrastructure. Traditional SaaS requires them to upload documents to the vendor's servers, creating:
•	Data exposure risk: Documents transit the public internet, are stored on third-party infrastructure, and are accessible to vendor employees.
•	Regulatory barriers: HIPAA, SOC 2, SOX, GDPR, and government classification requirements may prohibit cloud processing entirely.
•	Audit trail gaps: The user cannot independently verify what the vendor's system does with their data.
•	Vendor lock-in: Processed data, embeddings, and search indices are trapped in the vendor's ecosystem.
1.3	The Hardware Trajectory
NVIDIA CEO Jensen Huang, in his conversation with Lex Fridman, stated plainly: "Everything's going to be inference. We are solving the inference problem." Consumer hardware is rapidly approaching the capability threshold for running production AI models locally. The RTX 5090 delivers 32GB of VRAM -- sufficient to run state-of-the-art OCR, vision-language models, and embedding generators simultaneously. Within 2-3 hardware generations, mainstream consumer GPUs will exceed the requirements for most document AI workloads.
The question is not whether local inference will become widespread. It is how software companies can monetize it.
 
2.	The OCR Provenance Model: Local-First, Commercially Viable
OCR Provenance MCP Server answers that question with a new commercial architecture that inverts the traditional SaaS model:
Attribute	Traditional SaaS	OCR Provenance Model
Where software runs	Vendor's cloud	User's machine
Where data is processed	Vendor's servers	User's local GPU
What the user pays for	Access to hosted service	Per-page processing license
Vendor infrastructure cost	Compute + storage + bandwidth	Cloudflare Workers (effectively zero)
Data exposure	Full document upload to vendor	Zero -- data never leaves the machine
Vendor sees user data	Yes (by necessity)	No (by architecture)
Scaling cost for vendor	Linear with usage	Near-zero -- users provide compute
Revenue per user	Diminishes as infrastructure costs grow	Nearly 100% margin
The core insight is simple but radical: if the software runs on the user's hardware, the vendor has zero compute costs, zero storage costs, and zero bandwidth costs. Every dollar of revenue is nearly pure profit.
The system scales infinitely because each new user brings their own compute.
This is not freemium. This is not open-source with enterprise add-ons. This is a fully metered, commercially licensed product that happens to execute entirely on the user's machine.
 
3.	System Architecture
3.1	High-Level Architecture
OCR Provenance is a production-grade document intelligence platform that exposes 153 tools via the Model Context Protocol (MCP). It runs as three mandatory services inside a Docker container (or natively on bare-metal macOS/Linux):
AI Client (Claude Code, Cursor, Windsurf, Claude Desktop)
    |     | stdio / HTTP     v NPM Wrapper (packages/wrapper/)
    Docker lifecycle, HTTP-to-stdio bridge, AI client auto-registration
    |     | HTTP POST /mcp     v Docker Container (3 Mandatory Services)
    |
    +-- MCP Server (port 3366)
    |     153 tools, Document Viewer REST API, processing-mode endpoints
    |     Multi-session state (concurrent AI agent support)
    |
    +-- License Server (port 3000)
    |     Hono.js + SQLite, billing, auth, API key management
    |
    +-- Dashboard (port 3367)
          Next.js Web UI, account management, Stripe integration
    |
    +-- Python GPU Workers (Daemons)
          Marker-pdf OCR (8-10GB VRAM)
          Chandra VLM (10.6GB VRAM)
          Nomic/Legal Embedding (1-1.5GB VRAM)
          Cross-encoder Reranker (~1GB)
    |
    +-- SQLite + sqlite-vec + FTS5
          User databases, license server DB, viewer cache
3.2	Technology Stack
Layer	Technology
Protocol	Model Context Protocol (MCP) -- JSON-RPC over stdio or HTTP/SSE
Server	TypeScript 5.5 (ES2022), Node.js 20
Database	SQLite 3 + sqlite-vec (HNSW vector search) + FTS5 (full-text)
OCR Engine	Marker-pdf v1.10.2 (Python, CUDA-accelerated)
VLM Engine	Chandra OCR 2 v0.2.0 (Python, ~10.6GB VRAM)
Embeddings	Multi-domain: nomic-embed-text-v1.5 (general) or modernbert-legal-8192 (legal), 768-dim
Reranking	cross-encoder/ms-marco-MiniLM-L-12-v2 (GPU with CPU fallback)
License Server	Hono.js + SQLite (port 3000)
Dashboard	Next.js (port 3367)
Billing	Cloudflare Workers + D1 + Stripe
Container	Docker (two-image: models v3 ~14GB + app ~500MB) or bare-metal
Document Viewer	LibreOffice (Office to PDF), Pillow (TIFF to PNG), native PDF/image passthrough
3.3	Installation: One Command
The entire system installs with a single command: 
npx -y ocr-provenance-mcp install
This command:
1.	Pulls the Docker container (models base ~14GB + app ~500MB)
2.	Auto-provisions a license key (deterministic HMAC from machine ID)
3.	Starts all three services
4.	Auto-registers with Claude Code, Cursor, Windsurf, and Claude Desktop
5.	Outputs the dashboard URL
From cold start to fully operational: one command. No cloud account creation, no API key management, no configuration files.
3.4	Deployment Modes
Mode	Target	GPU	Installation
Docker (Linux/WSL)	Default for most users	NVIDIA CUDA	npx install
Mode	Target	GPU	Installation
Bare-metal macOS	Apple Silicon users	MPS (Metal)	npx install --mac
Bare-metal Linux	Non-Docker environments	CUDA/CPU	npx install --bare
Cloud GPU	Users without local GPU	Remote RTX 5090	Dashboard toggle
 
4.	The Billing Revolution: Zero-Overhead Monetization
This is the centerpiece of the OCR Provenance paradigm. The billing system is architecturally novel: it enables metered, per-page commercial software licensing for software that runs entirely on the user's machine, with the vendor having zero infrastructure overhead beyond a Cloudflare Worker.
4.1	The Dual-Layer Architecture
MCP Server           License Server         Cloudflare D1
(client-side)   -->  (port 3000, local) --> (source of truth)
                     SQLite cache            Stripe webhooks
LicenseClient        HMAC integrity          Balance tokens
The billing system operates as a dual-layer cache with a central source of truth:
1.	Local License Server (runs inside the user's Docker container on port 3000): Maintains a local SQLite cache of the user's balance, charges, and payments. Signs every balance mutation with HMAC. Communicates with the Cloudflare Worker for authorization.
2.	Cloudflare D1 (the source of truth): A serverless SQL database hosted on Cloudflare's edge network. Stores the authoritative balance, charge history, payment records, customer data, API keys, and machine secrets. All Stripe webhooks write here.
3.	Cloudflare Worker (the billing gateway): A serverless function at api.ocrprovenance.com that handles Stripe integration, balance authorization, secret bootstrapping, API key validation, and acts as a reverse proxy for the Cloud GPU API.
4.2	Per-Page Pricing
The system bills per page processed, not per document or per subscription period. Pricing is server-side (not hardcoded in the client) and varies by processing mode and embedding model:
Mode	General (nomic)	Legal (ModernBERT)
Local GPU	0.1 cents/page ($1 per 1,000 pages)	1 cent/page ($10 per 1,000 pages)
Cloud GPU	3 cents/page ($30 per 1,000 pages)	8 cents/page ($80 per 1,000 pages)
Free file types: .txt and .md files are free (no OCR models used).
Page counting happens BEFORE processing using precise methods:
•	PDF: PyMuPDF (fitz) for exact page count
•	TIFF: Pillow (n_frames) for multi-page count
•	DOCX: File size heuristic (~5KB per page)
•	PPTX: File size heuristic (~100KB per slide)
•	Images: 1 page each
•	Spreadsheets: 1 page each
4.3	The Charge Flow: How Money Moves
Every document processed triggers this precise sequence:
1.	Page Count   countDocumentPages(filePath, fileType) --> pageCount    amountCents = max(1, ceil(pageCount x costPerPage))    txt/md --> 0 pages --> free (no charge)
2.	Local Charge
   MCP calls POST /v1/charge on license server
   --> Deducts from local balance, signs new HMAC
   --> Returns: { charge_id, balance_cents, cost_cents }
3.	Central Authorization
   License server calls POST /v1/charge/authorize on Cloudflare Worker
   --> Deducts from D1 balance (source of truth)
   --> Returns: { authorized, balance_cents, balance_token }
4.	Processing
   On OCR success: POST /v1/confirm --> marks charge 'confirmed'
   On OCR failure: POST /v1/refund --> restores balance + signs HMAC
Failure modes:
   Central 402 --> refund local --> InsufficientBalanceError (doc stays 'pending')
   Network error --> refund local --> FATAL (no offline grace)
Critical design decision: There is NO offline grace period. The license server MUST be reachable for every charge. No cached balance processing, no chargeOffline() function. Network failure results in a FATAL error. This prevents balance manipulation via network disconnection.
4.4	Adding Funds: One-Click Stripe
The user experience for adding funds is deliberately frictionless:
1.	User clicks "Add $10" on the dashboard (preset buttons: $5, $10, $25, $50, or custom)
2.	Dashboard POSTs to license server, which calls the Cloudflare Worker
3.	Worker creates a Stripe hosted checkout session
4.	User completes payment on Stripe's page
5.	Stripe webhook fires to the Cloudflare Worker
6.	Worker upserts balance in D1 and the customers table
7.	Dashboard syncs balance immediately
8.	Balance available across ALL containers/machines linked to that email
4.5	HMAC Balance Integrity
Every balance mutation is cryptographically signed: signBalance(keyId, balanceCents) = HMAC-SHA256(LICENSE_SIGNING_KEY, "balance:{keyId}:{balanceCents}")
This signature is verified at EVERY balance mutation: charge, refund, credit, admin adjust, sync, stale refund, and startup migration. Verification uses constant-time comparison (timingSafeEqual()) to prevent timing attacks.
If the HMAC doesn't match, the system exits with a FATAL error. There is no recovery path from a tampered balance. This means:
•	Users cannot edit the SQLite database to inflate their balance
•	Memory corruption cannot silently change the balance
•	Any process that modifies the balance outside of the signed codepath kills the system
4.6	The Four-Point Identity Chain
Every user is identified by four linked credentials:
1.	Machine ID: Persisted to /data/.machine_id (survives container recreation)
2.	License Key: Deterministic HMAC from the machine ID (lk_ + HMAC-SHA256)
3.	Email: Captured on first Stripe payment
4.	API Key (for Cloud GPU): opk_ prefix + 32 hex characters
These four identifiers are cross-referenced. If any one is compromised or tampered with, the chain breaks and the system stops functioning. As stated by the developer: "If one of those four things is compromised in any way, the entire system shuts down."
4.7	Multi-Machine Balance Sharing
A single email address unifies balance across all machines:
•	User pays on Machine A -- balance reflects on Machine B (same email)
•	Machine dies -- new machine provisions new key with $0 balance
•	User authenticates with original email via magic link
•	System calls POST /v1/balance/consolidate-by-email
•	All old keys' balances consolidated to new key
•	Balance restored instantly
This is managed entirely through Cloudflare D1 -- the vendor never sees or stores document data, only email addresses and balance amounts.
4.8	Spending Controls
•	Monthly spending caps: Min $1, max $1,000
•	Enforcement: Charge blocked if monthly spend + cost exceeds limit
•	Dashboard: Warning at 80% of limit
•	Reset: Manual or automatic at month boundary
4.9	What the Vendor Sees vs. What the Vendor Does NOT See
Vendor Sees	Vendor Does NOT See
Machine ID (random hex)	Document content
License key hash	OCR output text
Email address	Embeddings
Balance (in cents)	VLM descriptions
Charge amounts	Search queries
SHA-256 file hashes	File names or paths
Payment amounts (via Stripe)	Database contents
API key hashes	Any processed data
The SHA-256 file hash is the only data point that has any relationship to the actual document, and it is a one-way hash that cannot be reversed. For fully air-gapped deployments, even this is eliminated.
4.10	The Vendor's Cost Structure
This is the revolutionary economics of the model:
Cost Item	Traditional SaaS	OCR Provenance
GPU compute	$2-8/hr per user session	$0 (runs on user's GPU)
Storage	$0.023/GB/month (S3)	$0 (user's local SQLite)
Bandwidth	$0.09/GB (egress)	$0 (all local)
Cloudflare Workers	N/A	~$0.50/million requests
Cloudflare D1	N/A	$0.75/million reads, first 25M free
Stripe fees	2.9% + $0.30 per txn	2.9% + $0.30 per txn
Infrastructure team	$200K+/year	$0 (serverless)
SOC 2/HIPAA compliance	$50K-200K/year	$0 (data never leaves user's machine)
Total marginal cost/user	$50-500/month	~$0.01/month
The result: revenue from every user is nearly 100% margin. The only costs are Cloudflare's serverless pricing
(pennies per million requests) and Stripe's transaction fee. There is no server to maintain, no database to scale, no GPU fleet to manage, no compliance infrastructure to build.
 
5.	Security Architecture: Air-Gapped by Default
5.1	Seven Security Layers
Layer 1: Network          -- 127.0.0.1 binding, Docker isolation
Layer 2: Authentication   -- X-License-Key header, session cookies, API keys (opk_)
Layer 3: Authorization    -- Rate limiting, spending limits, admin roles
Layer 4: Data Integrity   -- HMAC balances, Ed25519 signatures, SHA-256 chains
Layer 5: Secret Isolation -- Dashboard env whitelist, no secrets in workers
Layer 6: Input Validation -- Zod schemas, path sanitization, SQL parameterization
Layer 7: Audit Trail      -- Complete audit log with user/action/entity tracking
5.2	Zero Network Calls During Processing
A forensic audit of every source file confirms:
Worker	Network Imports	Network Calls	Verdict
OCR (Marker-pdf)	ZERO	ZERO	CLEAN
VLM (Chandra)	ZERO	ZERO	CLEAN
Embedding (nomic/legal)	ZERO	ZERO	CLEAN
GPU Utils	ZERO	ZERO	CLEAN
Reranker	ZERO	ZERO	CLEAN
Spreadsheet Prep	ZERO	ZERO	CLEAN
Docker-level enforcement ensures this cannot change at runtime:
•	HF_HUB_OFFLINE=1 -- prevents HuggingFace Hub HTTP requests
•	TRANSFORMERS_OFFLINE=1 -- prevents Transformers library downloads
•	All models (~14GB) baked into the Docker image at build time
•	Python workers communicate with TypeScript via stdin/stdout JSON, never HTTP
5.3	Ed25519 License Signing
Five cryptographic secrets protect the system:
Secret	Purpose	Risk if Leaked
SESSION_SECRET	Session HMAC	Session forgery
LICENSE_SIGNING_KEY	Ed25519 seed + HMAC balance	Balance tampering, key forgery
LICENSE_PUBLIC_KEY	Ed25519 public	Low (verification only)
WORKER_AUTH_SECRET	Cloudflare auth	Unauthorized charges
WEBHOOK_SHARED_SECRET	Stripe webhooks	Fake payment credits
Secrets are sourced via a 3-tier defense-in-depth strategy:
1.	Primary: Fetched from Cloudflare Worker (/v1/secrets/bootstrap), kept in memory only -- zero disk persistence
2.	Fallback: Disk cache at /data/.license_secrets (offline/air-gapped mode only)
3.	Last resort: Local Ed25519 generation (first boot only)
When Cloudflare is reachable, stale disk secrets are automatically deleted.
5.4	Secret Isolation
The dashboard is started with env -i (empty environment) plus a strict whitelist:
Dashboard Receives	Dashboard Does NOT Receive
SESSION_SECRET	LICENSE_SIGNING_KEY
OCR_LICENSE_KEY	LICENSE_PUBLIC_KEY
MCP_SERVER_URL	WORKER_AUTH_SECRET
OCR_LICENSE_SERVER	WEBHOOK_SHARED_SECRET
Even if the dashboard is fully compromised, the attacker cannot forge license tokens, manipulate balances, or intercept Stripe webhooks.
Python GPU workers receive NO secrets. They perform computation only.
5.5	Air-Gap Deployment
For organizations requiring mathematical proof of zero data egress:
docker run --network=none \   --cap-drop=ALL \
  --security-opt=no-new-privileges:true \
  --read-only \   -v /data:/data \
  leapable/ocr-provenance-mcp:latest
With --network=none, the container has zero network interfaces except loopback. The client can verify this independently:
docker exec <container> ip link show    # Only "lo" appears docker exec <container> ping 8.8.8.8   # "Network is unreachable"
In air-gap mode, billing is disabled (site license), eliminating ALL outbound network calls. The result is mathematically provable zero-egress processing.
5.6	Adversarial Testing
The system was hardened through adversarial testing: AI agent swarms were directed to attack the billing system, attempt balance manipulation, bypass licensing, and exfiltrate data. Vulnerabilities were discovered and fixed iteratively until the swarms could no longer find a path of compromise. Rate limiting, HMAC integrity checks, FATAL exits on tampering, and the four-point identity chain all emerged from this process.
 
6.	The Document Intelligence Platform
6.1	Document Lifecycle
File (PDF/DOCX/PPTX/XLSX/HTML/EPUB/TXT/CSV/MD)   |   v 1. INGESTION -- Validate, SHA-256 hash, dedup, create document record, count pages, charge
  |   v 2. OCR PROCESSING (Marker-pdf v1.10.2) -- Markdown + JSON blocks + metadata
  |
  +---> 3a. CHUNKING -- Section-aware, 2000 chars target, 10% overlap
  |       |
  |       v
  |     4a. EMBEDDING -- 768-dim vectors (nomic general or modernbert legal)
  |
  +---> 3b. IMAGE EXTRACTION -- PyMuPDF + file-based extraction           |           v         4b. VLM ANALYSIS (Chandra OCR 2) -- Structured JSON output           |           v         4c. VLM EMBEDDING -- Embed VLM descriptions   |   v 5. INDEXING -- sqlite-vec HNSW + FTS5 full-text + B-tree (50+ indexes)
  |   v 6. SEARCH -- BM25 + semantic + hybrid RRF + cross-encoder reranking
6.2	Provenance Chain
Every transformation creates an immutable provenance record linked by SHA-256 hash chains:
DOCUMENT (depth=0)           -- Original file
  +-- OCR_RESULT (depth=1)   -- Marker output
      +-- CHUNK (depth=2)    -- Text segments
      |   +-- EMBEDDING (depth=3) -- 768-dim vector
      +-- IMAGE (depth=2)    -- Extracted images
          +-- VLM_DESC (depth=3)   -- Chandra analysis
              +-- EMBEDDING (depth=4) -- VLM text vector
Each record carries:
•	content_hash: SHA-256 of the output
•	chain_hash: SHA-256 of content_hash + ":" + parent.chain_hash
•	processor, processor_version, processing_duration_ms
•	processing_quality_score, location (page, character range)
Tampering at any point in the chain breaks all downstream hashes. Verification walks from leaf to root, recomputing every hash. This provides a complete, cryptographically verifiable audit trail from the original file through every transformation to the final embedding.
6.3	Search Pipeline
QUERY (natural language)
  |
  +---> Query Classification (table? factual? concept?)
  +---> Query Expansion (synonyms, related terms)
  |
  +---> BM25 Search (FTS5, porter stemmer)
  +---> Semantic Search (embedding -> sqlite-vec cosine similarity)
  |
  +---> Hybrid Fusion (Reciprocal Rank Fusion, equal 1.0/1.0 weights)
        |
        +---> Result Enrichment (context chunks, provenance)
        +---> De-duplication
        +---> Cross-encoder Reranking (enabled by default)
The search system combines keyword precision (BM25) with semantic understanding (vector similarity), fused via
Reciprocal Rank Fusion and refined with a cross-encoder reranker. Results always include the original text (denormalized) and provenance links to the source document, enabling the AI to navigate from a search result to the full original file for complete context.
This is what makes RAG with provenance qualitatively different from standard RAG: when the AI retrieves a chunk, it knows exactly which document it came from, what page, what section, and can read the full context on demand. Out of a million documents, the search tells the AI exactly which six contain the relevant information -- with cryptographic proof of where that information originated.
6.4	Multi-Domain Embeddings
Databases are created with a specific embedding model chosen at creation time:
Model	Slug	Best For	VRAM
nomic-embed-text-v1.5	general	Business, research, general documents	~1 GB
modernbert-legal-8192	legal	Legal contracts, court filings, compliance	~1.5 GB
The model cannot be changed after creation (embeddings from different models are incompatible). This allows the system to optimize search quality for specific domains while maintaining a unified interface.
6.5	The 153 MCP Tools
The platform exposes 153 tools organized across 25 categories:
Category	Tools	Examples
Database Management	13	Create, list, select, stats, archive, search
Portability	7	Backup, clone, import, merge, restore, snapshot
Sharing	4	Share, import shared, transfer, receive
Ingestion	7	Ingest files, directory, process pending, retry failed
Search	7	Unified search, cross-database, RAG context
Documents	10	Get, list, delete, find similar, duplicates, structure
Chunks	4	Get, list, context expansion, page reading
Category	Tools	Examples
Embeddings	4	Get, list, rebuild, stats
Images	8	Get, list, search, pending, reanalyze
VLM	3	Describe, process, status
Provenance	6	Get chain, query, timeline, verify, export (W3C PROV)
Reports	7	Overview, performance, cost summary, error analytics
Intelligence	5	Guide, extras, tables, table export, recommendations
Comparison	6	Diff, batch compare, similarity matrix
Clustering	7	HDBSCAN/agglomerative/k-means, merge, reassign
Tags	6	Create, list, apply, remove, search, delete
Collaboration	11	Annotations, document locks, search alerts
Workflow	8	Submit, assign, review, approval chains
Contract/CLM	9	Extract, summarize, obligations, playbooks
Compliance	3	Reports, HIPAA, SOC 2/SOX export
Events	6	Webhooks, export annotations/audit/obligations
Config	2	Get and set configuration
Health	2	Integrity checks, maintenance
License	1	Balance and status
Dashboard	2	Open, status check
6.6	Document Viewer
A built-in REST API enables in-browser viewing of 18+ document types:
Input	Output	Method
PDF	Native PDF	Passthrough
DOCX, PPTX, XLSX, XLS, CSV	PDF	LibreOffice headless conversion
TIFF	PNG	Pillow conversion
Images (PNG, JPG, GIF, WebP, BMP)	Native	Passthrough
Markdown, Text	Native	Text rendering
The viewer manages a per-document cache with 24-hour auto-cleanup, concurrent request handling via flock, and path traversal hardening on all document IDs.
 
7.	GPU Processing: Leveraging End-User Inference
7.1	The Daemon Architecture
Three persistent daemon workers keep AI models loaded in VRAM, eliminating expensive model reload times:
Worker	Model	VRAM	Purpose
OCR	Marker-pdf v1.10.2	8-10 GB	PDF/DOCX/PPTX to structured
Markdown
VLM	Chandra OCR 2 v0.2.0	~10.6 GB	Image analysis with structured JSON output
Embedding	nomic/modernbert	1-1.5 GB	768-dimensional vector generation
Reranker	cross-encoder MiniLM	~1 GB	Search result reranking
Workers communicate via JSON lines on stdin/stdout -- no HTTP, no network. A global GPU mutex serializes concurrent processing pipelines to prevent VRAM OOM.
7.2	Performance Characteristics
Task	Input	Time	VRAM
OCR (10-page PDF)	PDF	15-30s	8-10 GB
OCR (160-page PDF)	PDF	~287s	8-10 GB
VLM image analysis	Single image	3-5s	~10.6 GB
Embed 100 chunks	Text array	200-400ms	1-1.5 GB
Rerank 20 passages	Query + passages	~50ms	~1 GB
7.3	Two-Image Docker Architecture
Image	Size	Rebuild Frequency
Models base (v3)	~14 GB	Rarely (model updates only)
App	~500 MB	Every release
The app image inherits from the models image, so model downloads happen once. All models are pre-cached -HF_HUB_OFFLINE=1 ensures zero runtime network calls.
 
8.	Cloud GPU Mode: Bridging the Hardware Gap
Not every user has an NVIDIA GPU with 12+ GB VRAM. Cloud GPU mode bridges this gap by offloading processing to a remote GPU server while maintaining the same local-first data architecture.
8.1	How It Works
User's machine                          GPU Server (operator's RTX 5090)
    |                                         |
    +-- Ingest files locally                  |
    +-- Process pending                       |
    |     |                                   |
    |     +-- Upload files ----------------->  |
    |     |                                   +-- OCR (Marker-pdf)
    |     |                                   +-- Chunking
    |     |   Real-time progress <-----------  +-- Embeddings
    |     |                                   +-- VLM analysis
    |     |                                   +-- Provenance chains
    |     +<-- Download result DB ----------  +-- VACUUM INTO portable DB
    |     |                                   |
    |     +-- Merge into local DB             |
    |     +-- Search immediately              |
    |                                         |
    +-- All search, export, analysis: local   |
Documents upload to the GPU server, get processed through the full pipeline, and the results merge back into the user's local database. The GPU server only holds files during processing and deletes them after delivery (24-hour TTL).
8.2	Cloud GPU Pricing
Model	Per Page	50-page PDF	1,000 pages
General (nomic)	3 cents	$1.50	$30
Legal (ModernBERT)	8 cents	$4.00	$80
8.3	Operator Economics
The GPU server is a simple REST API (Hono.js + SQLite job queue) exposed via Cloudflare Tunnel. The operator runs it on their own GPU hardware. Initial deployment uses the developer's personal RTX 5090 -- once adoption justifies it, the same code deploys unchanged to cloud H100 instances.
API keys are validated centrally via Cloudflare D1, meaning keys from any user's machine are recognized without the GPU server needing direct database access.
8.4	Seamless Switching
Users toggle between Local and Cloud on the dashboard Account page. The toggle writes a JSON config file -- no restart needed. Documents already processed are unaffected. Users can process some databases locally and others via cloud.
 
9.	Provenance and Compliance: Enterprise-Grade Trust
9.1	Why Provenance Matters
Standard RAG (Retrieval-Augmented Generation) has a trust problem: when an AI retrieves a text chunk and uses it to answer a question, there is no verifiable chain linking that chunk to the original source document. The chunk could have been modified, the source could be misidentified, and there is no way to audit the processing steps that produced it.
OCR Provenance solves this with SHA-256 hash chains that create an unbroken, cryptographically verifiable audit trail from the original file through every transformation to the final embedding. If any record in the chain is modified, the hash verification fails.
9.2	Compliance Exports
Tool	Standard
ocr_compliance_report	General compliance overview
ocr_compliance_hipaa	HIPAA-specific report
ocr_compliance_export	SOC 2, HIPAA, SOX format exports
ocr_provenance_export	W3C PROV format
Tool	Standard
ocr_export_audit_log	CSV/JSON audit trail
9.3	Governance Features
•	Annotations: Comments, corrections, questions, highlights, flags, approvals with threaded replies
•	Document Locks: Exclusive and shared locks with expiration
•	Workflow States: Draft -> submitted -> in_review -> changes_requested -> approved/rejected -> executed -> archived
•	Approval Chains: Template chains with ordered steps and per-document progress tracking
•	Audit Log: Every tool call logged with action, entity, user, session, IP, and details
9.4	Contract Lifecycle Management
The platform includes full CLM capabilities:
•	Obligation Tracking: Payment, delivery, notification, renewal, termination, compliance obligations with status and confidence scores
•	Playbook Comparison: Define preferred contract terms, compare against incoming contracts
•	Structured Extraction: Schema-driven data extraction from contract pages
 
10.	Scaling Economics: Why This Model Wins
10.1	The Vendor's Perspective
Traditional SaaS vendors face linear scaling costs: every new user requires proportionally more compute, storage, and bandwidth. OCR Provenance's local-first model inverts this:
Traditional SaaS:
  Revenue = Users x Price
  Cost = Users x (Compute + Storage + Bandwidth + Support)
  Margin = Revenue - Cost (decreasing with scale)
OCR Provenance:
  Revenue = Users x Pages x Price_per_page
  Cost = Cloudflare Workers (~$0.50/million requests) + Stripe (2.9% + $0.30)
  Margin = Revenue - (near zero) = ~100% of revenue
Every new user brings their own GPU, their own storage, and their own bandwidth. The vendor's marginal cost per user is effectively zero. The system scales infinitely because each user is a self-contained processing node.
10.2	The User's Perspective
Users benefit equally:
•	Privacy: Documents never leave their machine (local mode)
•	Performance: Processing on their own GPU, no network latency
•	Cost: $1 per 1,000 pages (local) vs. $10-50 per 1,000 pages (typical cloud OCR)
•	Control: Full data ownership, can export, backup, transfer databases
•	Compliance: Air-gap deployment satisfies the strictest regulatory requirements
•	No vendor lock-in: SQLite databases are portable, standard format
10.3	Hardware Trajectory Bet
The system is explicitly designed as a forward bet on consumer hardware improvement:
•	Today: RTX 5090 (32GB VRAM) -- runs the full pipeline
•	Minimum viable: 12GB VRAM GPU -- runs OCR + embedding (VLM optional)
•	Near future: As mainstream GPUs reach 12-16GB VRAM, the addressable market expands exponentially
In the meantime, Cloud GPU mode serves users without sufficient hardware. As hardware improves, users naturally migrate to local processing -- which is cheaper for them and has zero cost for the vendor.
10.4	Comparison to Traditional Pricing
Service	Cost per 1,000 pages
AWS Textract	$1.50-15.00
Google Document AI	$1.50-65.00
Azure Form Recognizer	$1.00-50.00
OCR Provenance (local)	$1.00 (general) / $10.00 (legal)
OCR Provenance (cloud)	$30.00 (general) / $80.00 (legal)
Local mode is price-competitive with the cheapest cloud services while offering complete privacy, provenance tracking, and 153 AI tools that the cloud services lack.
 
11.	The MCP Protocol: AI-Native Software Distribution
11.1	Why MCP, Not CLI or SDK
OCR Provenance is distributed as an MCP server, not a CLI tool or SDK. This is a deliberate architectural decision:
The problem with CLI: AI models must be trained on CLI documentation to use CLI tools effectively. A new product like OCR Provenance has no training data -- the AI would need to be prompted with full documentation every time.
The MCP advantage: The Model Context Protocol (MCP) provides machine-readable tool descriptions via JSON-RPC. When an AI client connects to the MCP server, it automatically discovers all 153 tools with their input schemas, descriptions, and validation rules. No training data needed. No documentation injection. The AI simply reads the tool catalog and uses it.
11.2	Multi-Session Support
Multiple AI agents can connect to the same MCP server simultaneously:
Session A --> SessionManager --> { currentDb: "legal", operations: 2 }
Session B --> SessionManager --> { currentDb: "research", operations: 0 }
Shared: ConnectionCache (ref-counted SQLite connection pool)
Isolated: database selection, operation counters, vector service cache
Each session is tracked via Mcp-Session-Id header with AsyncLocalStorage propagation. This enables concurrent multi-agent workflows -- one agent can be processing documents while another searches, with complete isolation.
11.3	AI Client Auto-Registration
The installer automatically detects and registers with:
•	Claude Code
•	Claude Desktop
•	Cursor
•	Windsurf
One npx install command and the user is ready to go in their preferred AI development environment.
 
12.	Case Study: Legal Document Intelligence
A practicing attorney presented 1,200 documents from a $90 million RICO case -- including 600+ page grand jury trial testimonies. The system:
1.	Ingested all 1,200 documents through the OCR pipeline, processing each page with provenance tracking
2.	Created searchable chunks with section-aware splitting, preserving table integrity and document structure
3.	Generated embeddings using the legal-domain model (modernbert-legal-8192), optimized for legal language
4.	Analyzed images (exhibits, signatures, stamps) via VLM with structured extraction
5.	Enabled semantic search across the entire corpus -- "which witnesses mentioned the offshore account?" returns the specific passages from the specific testimonies, with provenance links to the exact pages
The attorney then used AI to fill out witness analysis blueprints for all 60 witnesses, cross-referencing testimony with documentary evidence. The provenance chain ensured every AI-generated insight could be traced back to the original source document, page, and paragraph -- critical for court admissibility.
This use case is impossible without provenance. Standard RAG returns chunks with no verifiable source.
OCR Provenance returns chunks with cryptographically verifiable chains to the original filing.
 
13.	Future Implications
13.1	A Template for AI SaaS
OCR Provenance is not just a document processing tool -- it is a proof of concept for a new class of software company. The architecture is replicable:
1.	Build AI-powered software that runs on user hardware
2.	Meter usage via lightweight billing infrastructure (Cloudflare Workers + Stripe)
3.	Offer cloud fallback for users without sufficient hardware
4.	Scale with zero operational overhead
Any AI application that processes user data -- image generation, video editing, code analysis, medical imaging -- can adopt this model. The vendor provides the software and the billing infrastructure; the user provides the compute.
13.2	The Inference Economy
As Jensen Huang predicts, the future is inference. Consumer hardware will become increasingly capable of running sophisticated AI models. The companies that build commercial models around local inference -- rather than fighting the trend with cloud-only offerings -- will capture the next wave of AI adoption.
13.3	Enterprise Adoption Path
For enterprises requiring maximum security:
1.	Phase 1: Deploy with Docker --network=none for air-gapped processing
2.	Phase 2: Use provenance verification to build trust in the system
3.	Phase 3: Enable billing (site license or per-page) once approved by security team
4.	Phase 4: Scale across departments with multi-database isolation
The air-gap capability and provenance tracking address the two biggest barriers to enterprise AI adoption: data security and output trustworthiness.
 
14.	Conclusion
OCR Provenance MCP Server represents a paradigm shift in how AI-powered software can be built, distributed, and monetized. By running entirely on the user's hardware while maintaining commercial viability through lightweight metered billing, it eliminates the fundamental tension between data privacy and software-as-a-service.
The key innovations are:
1.	Zero-overhead metered billing via Cloudflare Workers and Stripe, with HMAC-protected balances and Ed25519 signed licenses
2.	Complete air-gap capability with mathematically provable zero data egress during processing
3.	SHA-256 provenance chains providing cryptographically verifiable audit trails from source document to embedding
4.	Dual-mode processing (local GPU + cloud GPU fallback) that meets users where they are today while positioning for the inference future
5.	MCP-native distribution enabling automatic AI tool discovery without training data
6.	Near-100% vendor margins through user-provided compute, with scaling limited only by adoption
This is not a theoretical model. It is a production system with 153 tools, 4,097+ tests, paying users, and a comprehensive security architecture validated through adversarial testing. It demonstrates that in the age of local AI inference, the most profitable SaaS model may be one where the vendor's servers never touch the user's data at all.
 
OCR Provenance MCP Server
Version 1.3.6 | 153 Tools | 4,097+ Tests | Schema v33
https://ocrprovenance.com
Author: Chris Royse | Kansas State University | arcibu@ksu.edu
This white paper was generated from the complete system documentation and architecture of OCR Provenance MCP Server, supplemented by insights from the developer on the commercial model and its implications for the future of SaaS.


ClipCannon: 0.961 Mean Cross-Encoder Speaker Similarity via Pipeline Engineering for Personalized Voice Cloning
Chris Royse
chrisroyseai@gmail.com
Abstract
I present ClipCannon, a personalized voice cloning pipeline that achieves a mean of 0.961 speaker similarity (SECS) on the WavLMForXVector cross-encoder across 10 novel sentences the target speaker never said, with individual samples reaching 0.975. These scores place the cloned voice firmly within the "same person, same session" verification band (0.95-0.99), meaning Microsoft's own speaker verification model cannot distinguish my clones from real same-session recordings of the target speaker. Built on Qwen3-TTS-12Hz-1.7B-Base with zero model modification, all gains come from pipeline engineering: Full In-Context Learning (ICL) mode with a real reference recording, WavLM-scored best-of-N candidate selection at temperature 0.3, and 50-clip centroid enrollment. For context, Microsoft declared "human parity" at 0.881 (VALL-E 2) and the current academic state-of-the-art is 0.891 (NaturalSpeech 3). My optimized mean of 0.961 exceeds the "human parity" threshold by +0.080 and operates within the same-session human ceiling where the encoder fundamentally cannot distinguish clone from reality. Additionally, Microsoft's DNSMOS naturalness predictor scores my clones identically to the speaker's real recordings (3.93 vs 3.93), confirming zero perceptible degradation in speech quality.
1.	Introduction
Voice cloning systems aim to generate speech that sounds like a specific target speaker. The field has progressed rapidly, with zero-shot systems like VALL-E [1], NaturalSpeech 3 [2], and VALL-E 2 [3] achieving speaker similarity scores of 0.85-0.89 on standard cross-encoder benchmarks. These systems are evaluated on their ability to clone strangers from a single short reference clip.
I address a different but practically important problem: personalized voice cloning, where the target speaker has provided reference recordings and the goal is to produce verification-grade clones that independent speaker verification systems cannot distinguish from real recordings.
1.1	Understanding the SECS Scale
Speaker Encoder Cosine Similarity (SECS) measures how similar two audio recordings sound to a speaker verification model. The scale has natural reference points established by real human speech:
WavLM SECS Range	What It Represents
1.000	Identical audio file (bit-for-bit same recording)
0.95-0.99	Same person, same microphone, same session (natural ceiling)
0.85-0.95	Same person, different session or microphone
0.70-0.85	Same person, very different recording conditions
< 0.70	Likely different speakers
The "same person, same session" band (0.95-0.99) represents the highest confidence tier of speaker verification. Two recordings of the same person saying different words in the same session with the same microphone typically score within this range. My pipeline produces clones that score within this band (0.961 mean, 0.975 max), meaning the speaker verification system classifies my clones with the same confidence as real same-session recordings.
Microsoft's VALL-E 2 declared "human parity" at 0.881 -- well below the same-session band, in the "different session" range. My pipeline exceeds this by +0.080.
1.2	Key Contribution
My key finding is that pipeline engineering -- the orchestration of reference selection, generation mode, candidate scoring, post-processing, and enrollment strategy -- contributes more to clone quality than model architecture when the target speaker is known. I achieve 0.961 mean cross-encoder SECS using an unmodified Qwen3-TTS model, placing clones inside the human same-session verification band and exceeding all published zero-shot results.
2.	Related Work
Zero-shot voice cloning. VALL-E [1] pioneered neural codec language models for TTS, achieving 0.849 SECS on LibriSpeech. NaturalSpeech 3 [2] uses latent diffusion with factorized codec for 0.891 SECS. VALL-E 2 [3] introduced Repetition Aware Sampling and achieved 0.881 SECS, claiming "human parity." MaskGCT [4] and F5-TTS [5] achieve 0.877 and 0.862 respectively using non-autoregressive architectures.
Personalized TTS. YourTTS [6] introduced Speaker Consistency Loss for multi-speaker TTS. LoRP-TTS [7] uses low-rank adapters for personalization. The Blizzard Challenge evaluates personalized TTS through large-scale listening tests.
Speaker verification. WavLM [8] provides self-supervised representations for speaker verification. The WavLMForXVector model (microsoft/wavlm-base-plus-sv) fine-tuned on VoxCeleb is used by ClonEval [9] as the standard benchmark encoder.
Audio enhancement. Resemble Enhance [10] uses latent conditional flow matching for speech denoising and bandwidth extension.
3.	Method
3.1	Pipeline Architecture
My pipeline consists of five stages applied sequentially:
1.	Reference Recording: A 5-15 second recording of the target speaker saying any sentence, captured on their actual microphone.
2.	Full 	ICL 	Prompt 	Construction: 	The 	reference 	audio 	and 	its 	transcript 	are 	provided 	to
Qwen3-TTS-12Hz-1.7B-Base as an in-context learning prompt (x_vector_only_mode=False). This allows the model to copy not just speaker identity (WHO) but speaking style (HOW) -- accent, cadence, mic character, and room tone.
3.	WavLM-Scored Best-of-N Generation: N candidates (default 12) are generated with different random seeds at temperature 0.3, top_p 0.85, repetition_penalty 1.05, and max_new_tokens 2048. Each candidate is scored against a WavLM centroid of the speaker's real voice. The highest-scoring candidate is selected. Critically, candidates are scored with the same encoder used for benchmark evaluation (WavLMForXVector), not a different encoder. My ablation shows that scoring with a mismatched encoder actively selects worse candidates for the target metric.
4.	50-Clip Centroid Enrollment: The WavLM reference embedding is a centroid (average) of x-vectors extracted from 50 reference clips, L2-normalized. The centroid is built from the speaker's highest-quality clips ranked by WavLM similarity to the real mic recording. This produces a more robust enrollment than single-reference scoring.
5.	Multi-Gate Verification: Three quality gates validate the output: (a) Sanity -- duration ratio, clipping, SNR, silence gaps; (b) Intelligibility -- Word Error Rate via Whisper transcription with punctuation-stripped comparison; (c) Identity -- SECS threshold against the reference embedding.
Optional: Resemble Enhance denoise post-processing removes metallic TTS codec artifacts and upsamples from 24kHz to 44.1kHz for broadcast-quality delivery. This has negligible effect on WavLM SECS (+0.001) but improves perceptual quality.
3.2	Critical Design Decisions
Full ICL vs x-vector only. The most impactful design decision is using Full ICL mode. In x-vector only mode, the model receives only the speaker embedding and generates speech with correct identity but potentially wrong accent, cadence, or prosody. In Full ICL mode, the model additionally receives the reference audio waveform and its transcript, enabling it to copy the complete speaking style. My ablation shows Full ICL provides the largest single improvement in cross-encoder SECS.
Score with the target encoder. My ablation revealed that selecting candidates with a mismatched encoder (Qwen3 2048-dim when benchmarked on SpeechBrain 192-dim) actually reduces the target metric by -0.013. The two encoders have only 0.51 correlation on TTS audio. Scoring candidates with the same encoder used for evaluation eliminates this anti-correlation.
Temperature 0.3 for WavLM optimization. A temperature sweep across 0.2-0.8 revealed that temperature 0.3 maximizes WavLM SECS (mean 0.932 vs 0.911 at 0.7). Lower temperature produces more consistent spectral characteristics, and WavLM's x-vector head heavily weights mid-level acoustic features (layers 3-5) that capture timbre and spectral envelope -- properties that benefit from spectral consistency.
Real reference recording. The ICL reference must be a real microphone recording of the target speaker, not a processed training clip. Training clips that have undergone source separation (e.g., Demucs) lose acoustic characteristics that both the TTS model and the evaluation encoder rely on.
3.3	Voice Fingerprinting
A 2048-dimensional speaker embedding (voice fingerprint) is built from the target speaker's reference clips using marksverdhei/Qwen3-Voice-Embedding-12Hz-1.7B, an ECAPA-TDNN encoder in the same model family as Qwen3-TTS's internal speaker encoder. This fingerprint is used for the identity verification gate and for the matched-encoder SECS measurement. For WavLM benchmark evaluation, a separate 512-dimensional WavLM centroid is used.
4.	Experimental Setup
4.1	Target Speaker
A single English male speaker with 489 training clips extracted from video recordings via Demucs source separation and silence-boundary splitting. Two clean microphone recordings (8-9 seconds each) serve as ICL references and ground truth for scoring.
4.2	Evaluation Encoders
Encoder	Dimension	Training Data	Relationship to Pipeline
WavLMForXVector
(microsoft/wavlm-base-plus-sv)	512	VoxCeleb	None (ClonEval standard)
SpeechBrain ECAPA-TDNN
(spkrec-ecapa-voxceleb)	192	VoxCeleb	None (academic standard)
Qwen3-TTS ECAPA-TDNN
(Qwen3-Voice-Embedding-12Hz-	2048
1.7B)	Internal Qwen data	Same model family as TTS
4.3	Human Baseline
I establish the real-human reference band by scoring the target speaker's own recordings against each other on WavLM:
Comparison	WavLM SECS
Same session, same mic (recording 1 vs recording 2)	0.999
Different session, different mic (mic vs source video)	0.718
The same-session band (0.95-0.99) is the natural ceiling. Scores within this band indicate the encoder cannot distinguish the audio from a real same-session recording.
4.4	Test Protocol
All test sentences are novel content the speaker never said. 10 diverse English sentences covering casual, professional, and descriptive speech. The ICL reference says "OCR Provenance MCP server is the best AI memory system in existence." Test sentences have zero lexical overlap with the reference. Each sentence generates 12 candidates; the WavLM-highest is selected.
5.	Results
5.1	WavLM-Optimized Benchmark (Primary Result)
Metric	Value
Mean WavLM SECS (10 novel sentences, best-of-12)	0.961
Std dev	0.017
Min	0.914
Max	0.975
Qwen3 SECS mean (for reference)	0.987
All 10 individual scores except one (0.914) fall within the 0.95-0.99 same-session human band.
5.1.1	DNSMOS Quality Assessment
Microsoft's DNSMOS P.835 predictor confirms zero naturalness degradation:
Audio	P808 (Naturalness)	OVRL (Overall)	SIG (Signal)	BAK (Background)
Real mic recording	3.93	3.36	3.60	4.16
Clone mean (14 samples)	3.93	3.32	3.58	4.10
Clone best	4.22	3.47	3.66	4.19
The DNSMOS naturalness score (P808) is identical between real and cloned speech (3.93), with the best individual clone (4.22) scoring higher than the real recording.
5.1.2	UTMOS Naturalness Assessment
UTMOS [11], the VoiceMOS Challenge 2022 winning system and gold-standard automated MOS predictor, independently confirms the same finding:
Audio	UTMOS Score
Real mic recording	2.997
Clone mean (14 samples)	3.012
Clone best	3.024
The clone scores marginally higher (+0.015) than real speech on UTMOS. Two independent quality predictors (DNSMOS and UTMOS) both confirm that the cloning pipeline adds zero measurable degradation in perceived naturalness. The clone is not merely "as good as" real speech -- it is indistinguishable from it by automated quality assessment.
5.2	Per-Sentence Breakdown
#	WavLM SECS	Sentence
0	0.914	The weather outside is absolutely beautiful today.
1	0.964	Can you believe how fast technology is advancing...
2	0.954	The customer feedback has been overwhelmingly positive...
3	0.975	Running every morning has completely changed my energy...
4	0.968	The presentation went really well and the client...
5	0.972	I just finished reading a really interesting book...
6	0.965	The meeting has been rescheduled to three o'clock...
7	0.964	I need to discuss the quarterly budget before...
8	0.964	The traffic on the highway was absolutely terrible...
9	0.969	I'm really excited about the upcoming product launch...
5.3	Comparison with Published Results
System	Cross-Encoder SECS	vs Human Parity	Evaluation Paradigm
ClipCannon (mean)	0.961	+0.080	Personalized, WavLM-scored best-of-12
ClipCannon (max)	0.975	+0.094	Personalized, WavLM-scored best-of-12
NaturalSpeech 3 [2]	0.891	+0.010	Zero-shot, single generation
VALL-E 2 [3] ("human parity")	0.881	baseline	Zero-shot, single generation
MaskGCT [4]	0.877	-0.004	Zero-shot, single generation
F5-TTS [5]	0.862	-0.019	Zero-shot, single generation
ElevenLabs	~0.80	-0.081	Commercial API
Note: Academic systems use zero-shot evaluation on LibriSpeech (strangers, single reference clip, single generation). ClipCannon uses personalized pipeline with ICL reference + WavLM-scored best-of-12 on a known speaker. These are different evaluation paradigms.
5.4	Temperature Sweep
Temperature	WavLM Mean	WavLM Max
0.2	0.923	0.941
0.3	0.932	0.963
0.4	0.917	0.922
0.5	0.930	0.960
0.6	0.917	0.942
0.7	0.909	0.936
0.8	0.911	0.952
Temperature 0.3 maximizes mean WavLM SECS, consistent with WavLM's heavy weighting of mid-level spectral features that benefit from generation consistency.
5.5	Enrollment Strategy
Enrollment Method	WavLM Mean	WavLM Max
Single reference	0.924	0.946
Enrollment Method	WavLM Mean	WavLM Max
2-mic centroid	0.924	0.946
50-clip centroid (best clips)	0.939	0.950
50-clip centroid enrollment improves mean SECS by +0.015 over single reference, as the averaged centroid reduces enrollment noise.
5.6	Iterative Refinement
Using the best round-1 TTS output as the ICL reference for round 2 produced worse results (0.944 mean vs 0.961), confirming that TTS output degrades when used as its own reference. The model copies its codec artifacts, creating a feedback loop.
5.7	Reference Scaling
Reference Clips	Qwen3 SECS
1	0.964
5	0.981
25	0.986
50	0.986
250	0.987
489	0.982
5.8	Ablation Study (SpeechBrain Cross-Encoder)
Pipeline Component	Qwen3 SECS	SpeechBrain SECS	Delta (SB)
A: Zero-shot (1 clip, x-vector, temp=0.8)	0.971	0.530	baseline
C: + Full ICL mode	0.975	0.624	+0.094
D: + Best-of-8 (Qwen3-scored)	0.977	0.611	-0.013
E: + Resemble Enhance denoise	0.975	0.642	+0.031
Full ICL mode is the single most impactful component (+0.094 SpeechBrain SECS). Best-of-N scored by a mismatched encoder hurts the target metric (-0.013).
6.	Discussion
Same-session verification band. My mean score of 0.961 places clones within the WavLM same-session band (0.95-0.99). This is significant because it means the speaker verification system treats my clones with the same confidence level as two real recordings of the target speaker made in the same session with the same microphone. At this score, the encoder fundamentally cannot distinguish clone from reality in the same-session context.
Pipeline engineering vs model architecture. My results demonstrate that for personalized voice cloning, the orchestration of existing components (ICL mode, target-encoder candidate selection, enrollment strategy, temperature optimization) contributes more than the underlying model architecture. An unmodified Qwen3-TTS model, combined with principled pipeline design, exceeds purpose-built research systems.
Score with the right encoder. The most overlooked optimization is scoring candidates with the same encoder used for evaluation. Encoders disagree significantly on TTS audio (0.51 correlation between Qwen3 and WavLM). Selecting candidates with a mismatched encoder is nearly random from the evaluation encoder's perspective.
Evaluation paradigm differences. Academic systems are evaluated on zero-shot cloning of strangers from single clips. My system is purpose-built for known speakers with reference data. These are fundamentally different tasks, and direct numerical comparison should be interpreted accordingly.
The ICL reference as the critical variable. My analysis revealed that the acoustic quality of the ICL reference recording is the dominant factor in cross-encoder scores. References from real microphone recordings produce clones scoring in the 0.95+ range, while references from source-separated training clips produce 0.40-0.55. The TTS model copies not just the speaker identity but the full acoustic character of the reference.
7.	Conclusion
I present a personalized voice cloning pipeline that achieves 0.961 mean WavLM cross-encoder SECS (0.975 max) across 10 novel sentences, placing clones within the human same-session verification band (0.95-0.99). This exceeds the declared "human parity" threshold of 0.881 by +0.080, meaning the benchmark encoder cannot distinguish my clones from real same-session recordings. DNSMOS naturalness assessment confirms zero quality degradation (clone: 3.93, real: 3.93 on P808). All gains come from pipeline engineering over an unmodified Qwen3-TTS model. I release the pipeline methodology and benchmark results to enable reproduction.
References
[1]	Wang et al., "VALL-E: Neural Codec Language Models are Zero-Shot Text to Speech Synthesizers," arXiv:2301.02111, 2023.
[2]	Tan et al., "NaturalSpeech 3: Zero-Shot Speech Synthesis with Factorized Codec and Diffusion Models," arXiv:2403.03100,2024.
[3]	Chen et al., "VALL-E 2: Neural Codec Language Models are Human Parity Zero-Shot Text to Speech Synthesizers,"arXiv:2406.05370, 2024.
[4]	Wang et al., "MaskGCT: Zero-Shot Text-to-Speech with Masked Generative Codec Transformer," arXiv:2409.00750, 2024. [5] Chen et al., "F5-TTS: A Fairytaler that Fakes Fluent and Faithful Speech with Flow Matching," arXiv:2410.06885, 2024.
[6]	Casanova et al., "YourTTS: Towards Zero-Shot Multi-Speaker TTS and Zero-Shot Voice Conversion for Everyone," ICML2022.
[7]	LoRP-TTS, "Low-Rank Personalized Text-to-Speech," arXiv:2502.07562, 2025.
[8]	Chen et al., "WavLM: Large-Scale Self-Supervised Pre-Training for Full Stack Speech Processing," IEEE JSTSP, 2022.
[9]	Christop et al., "ClonEval: An Open Voice Cloning Benchmark," arXiv:2504.20581, 2025.
[10]	Resemble AI, "Resemble Enhance: Speech Denoising and Enhancement," github.com/resemble-ai/resemble-enhance, 2024.
[11]	Saeki et al., "UTMOS: UTokyo-SaruLab System for VoiceMOS Challenge 2022," Interspeech 2022, arXiv:2204.02152.

ClipCannon Technical White Paper v0.1.0
Page 1
ClipCannon
A Multi-Modal Embedding Architecture for AI-Driven Video Understanding,
Editing, and Voice Synthesis
Version 0.1.0 — Technical White Paper
March 2026
ClipCannon Technical White Paper v0.1.0
Page 2
Abstract
ClipCannon is an AI-powered video understanding, editing, rendering, and voice synthesis
pipeline that ingests raw video footage and transforms it into platform-optimized clips through a
22-stage directed acyclic graph (DAG) analysis pipeline. The system employs eight distinct
embedding models across four perceptual modalities — visual, linguistic, acoustic, and speaker
identity — to construct a dense, queryable representation of video content. These embeddings
are stored in a per-project SQLite database augmented with the sqlite-vec extension for sublinear nearest-neighbor search, enabling an AI assistant to discover semantically meaningful
moments, detect natural edit boundaries, and compose edits with cross-stream intelligence. A
separate real-time Voice Agent subsystem extends the architecture with a ninth embedding
model for speaker verification during voice cloning. This paper describes the complete system
architecture, details each embedding model and its role in the pipeline, explains how
embeddings interact during discovery and editing, and documents the provenance, billing, and
rendering subsystems that make ClipCannon a production-grade tool for AI-assisted video
production.
Table of Contents
1. Introduction
2. System Architecture
3. The 22-Stage Analysis Pipeline
4. Multi-Modal Embedding Architecture
4.1 Visual Embeddings (SigLIP)
4.2 Semantic Embeddings (Nomic Embed)
4.3 Emotion Embeddings (Wav2Vec2)
4.4 Speaker Embeddings (WavLM)
4.5 Voice Identity Embeddings (Qwen3-TTS ECAPA-TDNN)
5. Vector Storage and Retrieval
6. Cross-Stream Intelligence
7. Narrative Analysis via Large Language Model
8. Scene Analysis and Layout Computation
9. The Editing Engine
10. The Rendering Engine
11. The Audio Engine
12. The Voice Engine
13. The Avatar Engine
14. The Voice Agent
15. Provenance and Data Integrity
16. Billing and Credit System
17. GPU Management and Resource Optimization
18. MCP Protocol Integration
19. Database Architecture
20. Testing and Verification
21. Graceful Degradation
ClipCannon Technical White Paper v0.1.0
Page 3
22. Conclusion
Appendix A: Model Summary Table
Appendix B: Embedding Dimension Reference
ClipCannon Technical White Paper v0.1.0
Page 4
1. Introduction
The explosion of short-form video content across platforms such as TikTok, Instagram Reels,
YouTube Shorts, and LinkedIn has created an acute demand for tools that can rapidly analyze
long-form source footage and extract platform-optimized clips. Traditional video editing requires
manual scrubbing, subjective judgment about “interesting moments,” and significant technical
skill in encoding, aspect ratio management, and caption placement. ClipCannon addresses
these challenges by constructing a rich, multi-modal understanding of video content through
neural embeddings and then exposing that understanding to AI assistants via the Model Context
Protocol (MCP).
The central insight behind ClipCannon is that video is inherently multi-modal: a single frame
carries visual semantics, the audio track conveys emotion and speaker identity, the transcript
carries linguistic meaning, and the temporal structure of these signals reveals narrative flow. By
embedding each modality into a separate vector space and storing these embeddings alongside
structured metadata in a per-project database, ClipCannon enables an AI assistant to perform
cross-modal queries — for example, finding moments where emotional intensity peaks coincide
with topic transitions and natural audio boundaries.
ClipCannon operates as three coordinated processes — an MCP server exposing 51 tools, a
license server managing credit-based billing, and a web dashboard for project oversight — plus
an optional real-time Voice Agent for conversational interaction. The entire system runs locally,
with no cloud API dependencies for core functionality. All ML models execute on the user’s
GPU, and all data remains on-premises in SQLite databases.
2. System Architecture
ClipCannon is composed of four independent processes:
Process Port / Transport Technology Purpose
MCP Server stdio (or SSE
3366)
mcp SDK + Python
asyncio
51 tools for AI assistants
License Server 3100 FastAPI + Uvicorn Credit billing with HMAC
integrity
Dashboard 3200 FastAPI + static HTML Web UI for projects,
credits, provenance
Voice Agent 8765 (WS) or
local mic
Pipecat + PyAudio Real-time conversational
AI
The MCP server is the primary interface. An AI assistant (such as Claude) connects via stdio
transport and invokes tools to create projects, analyze video, discover moments, compose edits,
render clips, synthesize voice, and generate lip-synced avatars. The server is stateless; all
persistent state lives in per-project SQLite databases under
~/.clipcannon/projects/{project_id}/analysis.db.
ClipCannon Technical White Paper v0.1.0
Page 5
2.1 Source Code Organization
src/
 clipcannon/ # Core package (included in wheel)
 pipeline/ # 22-stage analysis DAG
 editing/ # EDL engine, captions, smart crop, motion, overlays
 rendering/ # FFmpeg-based async rendering with 7 platform profiles
 audio/ # AI music, MIDI, DSP SFX, mixing, cleanup
 voice/ # Voice cloning: data prep, synthesis, verification, enhancement
 avatar/ # LatentSync lip-sync diffusion
 tools/ # 51 MCP tool definitions and dispatchers
 db/ # SQLite connection factory, schema, queries
 gpu/ # Precision detection, LRU model manager
 provenance/ # SHA-256 hash chain, tamper detection
 billing/ # HMAC-signed credits, license client
 dashboard/ # FastAPI web UI with 7 route groups
 license_server/ # Credit billing FastAPI service
 voiceagent/ # Standalone real-time voice assistant
2.2 Technology Stack
The system is built on Python >= 3.12 with Pydantic >= 2.0 for data validation, SQLite for
storage (with sqlite-vec for vector KNN), and PyTorch >= 2.3.0 for GPU inference. Video
processing uses FFmpeg via async subprocess invocations, with NVENC hardware
acceleration where available and automatic software fallback.
3. The 22-Stage Analysis Pipeline
When a user invokes clipcannon_ingest, ClipCannon executes a directed acyclic graph (DAG)
of 22 pipeline stages. The DAG is topologically sorted using Kahn’s algorithm, producing
execution levels where stages within a level share no mutual dependencies and execute
concurrently via asyncio.gather().
3.1 DAG Topology
Level 0: probe
Level 1: vfr_normalize -> probe
Level 2: audio_extract, frame_extract -> vfr_normalize
Level 3: source_separation, visual_embed, ocr, -> audio_extract / frame_extract
 quality, storyboard, transcribe, acoustic
Level 4: shot_type, scene_analysis, semantic_embed, -> frame_extract, visual_embed,
 narrative_llm, speaker_embed, transcribe, audio_extract
 emotion_embed, reactions, profanity
Level 5: chronemic -> transcribe, speaker_embed
Level 6: highlights -> 7 upstream stages
Level 7: finalize -> 15 upstream stages
3.2 Required vs. Optional Stages
Six stages are required — failure of any required stage aborts the pipeline:
1. probe — FFprobe metadata extraction, VFR detection, SHA-256 hashing
2. vfr_normalize — Variable-to-constant frame rate conversion via FFmpeg
3. audio_extract — Extracts 16kHz mono (for speech models) and native-rate audio
4. frame_extract — Extracts JPEG frames at configurable FPS (default 2)
ClipCannon Technical White Paper v0.1.0
Page 6
5. transcribe — Speech-to-text with word-level timestamps
6. finalize — Status reconciliation, provenance verification, cleanup
The remaining 16 stages are optional. They employ graceful degradation: if an optional stage
fails, downstream stages check the dependency status and either use fallback data or skip
gracefully. The project is still marked as “ready” provided the provenance chain verifies.
3.3 Audio Source Resolution
A shared utility, resolve_audio_input(), provides cascading audio path selection. If source
separation produced a vocals.wav stem (isolating speech from background music), downstream
audio stages prefer it. If source separation failed or was not run, stages fall back to
audio_16k.wav. This ensures the pipeline completes even without GPU-intensive source
separation.
4. Multi-Modal Embedding Architecture
ClipCannon’s analytical power derives from its use of multiple specialized embedding models,
each trained for a distinct perceptual modality. These models transform raw sensory data —
pixels, audio waveforms, text — into dense vector representations in learned metric spaces
where semantic similarity corresponds to geometric proximity.
The system employs nine distinct embedding models across five embedding spaces:
4.1 Visual Embeddings — SigLIP-SO400M (1152 dimensions)
• Model: google/siglip-so400m-patch14-384 (Google)
• Dimension: 1152 (float32)
• Storage: vec_frames virtual table (sqlite-vec)
• Pipeline Stage: visual_embed (Level 3)
• Input: Extracted JPEG frames at configured FPS
SigLIP (Sigmoid Language-Image Pre-training) is a vision-language model trained with a
sigmoid loss function on image-text pairs. Unlike CLIP’s contrastive softmax loss, SigLIP’s
sigmoid formulation enables each image-text pair to be treated as an independent binary
classification, improving scalability and visual grounding quality.
Processing Pipeline:
1. Frames are loaded and preprocessed to 384x384 resolution via the SigLIP processor.
2. The vision encoder produces 1152-dimensional embeddings per frame.
3. Embeddings are L2-normalized for cosine similarity computation.
4. Frames are processed in configurable batches (default 64) for GPU efficiency.
5. Embeddings are inserted into the vec_frames sqlite-vec table with frame path,
timestamp, and project ID metadata.
Scene Detection: After embedding all frames, the system computes pairwise cosine similarities
between consecutive frame embeddings. A new scene boundary is detected when similarity
drops below a configurable threshold (default 0.75). This approach detects semantic scene
changes rather than just visual discontinuities — a gradual topic shift in a single-camera
ClipCannon Technical White Paper v0.1.0
Page 7
interview is detected because the visual content (gestures, props, expressions) changes
semantically even when the camera angle remains constant.
Dual Use for Shot Classification: The same SigLIP model is reused in the shot_type stage
(Level 4) for zero-shot image-text classification. Each scene’s key frame is compared against
five text prompts describing shot types (extreme close-up, close-up, medium, wide,
establishing), and the highest-similarity label is assigned. This eliminates the need for a
separate shot classification model while leveraging SigLIP’s strong vision-language alignment.
Color Extraction: For each detected scene, the system also extracts dominant colors via PIL’s
median-cut quantization algorithm on a 64x64 thumbnail, producing the top 5 hex color values
for visual metadata.
4.2 Semantic Embeddings — Nomic Embed Text v1.5 (768 dimensions)
• Model: nomic-ai/nomic-embed-text-v1.5 (Nomic AI)
• Dimension: 768 (float32)
• Storage: vec_semantic virtual table (sqlite-vec)
• Pipeline Stage: semantic_embed (Level 4)
• Input: Transcript segments from the transcript_segments table
Nomic Embed is a text embedding model designed for semantic search and retrieval. It
produces high-quality sentence embeddings that cluster semantically related content in vector
space.
Processing Pipeline:
1. Transcript segments are prefixed with "search_document: " (Nomic’s recommended
prefix for document encoding) to align with the model’s training distribution.
2. Segments are batch-encoded via sentence-transformers.SentenceTransformer,
producing 768-dimensional L2-normalized embeddings.
3. Embeddings are inserted into the vec_semantic sqlite-vec table with segment ID,
timestamp, and transcript text.
Topic Clustering: After embedding all segments, the system performs agglomerative clustering
with distance threshold 1.2, cosine metric, and average linkage. For each discovered topic
cluster, the system computes:
• Keywords: Top-5 words by frequency (after stop-word removal)
• Coherence: Average pairwise cosine similarity within the cluster
• Semantic density: 1 / (1 + mean_distance_from_centroid), measuring how tightly the
cluster’s segments relate to each other
These topic records are stored in the topics table and directly feed the highlight scoring system,
where semantic density is weighted at 0.20 in the composite score.
Semantic Search: The clipcannon_search_content MCP tool uses the vec_semantic table for
KNN nearest-neighbor search. User queries are embedded with the "search_query: " prefix and
compared against stored segment embeddings, enabling natural-language search across video
content (e.g., “find where they talk about authentication”).
4.3 Emotion Embeddings — Wav2Vec2 (1024 dimensions)
• Model: facebook/wav2vec2-large-960h (Meta/Facebook)
ClipCannon Technical White Paper v0.1.0
Page 8
• Dimension: 1024 (float32)
• Storage: vec_emotion virtual table (sqlite-vec)
• Pipeline Stage: emotion_embed (Level 4)
• Input: Audio waveform (16kHz mono, from vocals.wav or audio_16k.wav)
Wav2Vec2 is a self-supervised speech representation model originally trained for automatic
speech recognition. ClipCannon repurposes its latent representations as a proxy for emotional
content, exploiting the fact that Wav2Vec2’s hidden states encode prosodic features (pitch
contour, energy dynamics, speaking rate variations) that correlate with emotional expression.
Processing Pipeline:
1. Audio is loaded and resampled to 16kHz mono.
2. The waveform is segmented into 5-second windows with 2.5-second stride (50%
overlap).
3. Each window is processed through Wav2Vec2’s encoder.
4. From the last hidden state (shape: [1, T, 1024]), three scalar signals are derived:
Energy: RMS / (RMS + 1); Arousal: variance / (variance + 1); Valence: 0.5 + mean /
(|mean| + 2).
5. The full 1024-dimensional mean-pooled hidden state is stored as the emotion
embedding.
6. Scalar signals are inserted into the emotion_curve table; embeddings into vec_emotion.
Fallback Mechanism: If Wav2Vec2 fails to load (insufficient VRAM, missing dependencies), the
system falls back to raw audio RMS analysis: Energy = rms * 5.0 clipped to [0, 1]; Arousal =
sub-window energy variance; Valence = fixed at 0.5; Embedding = 1024-dimensional zero
vector. This ensures the emotion curve is always populated, even without the ML model,
preserving downstream highlight scoring at reduced fidelity.
4.4 Speaker Embeddings — WavLM (512 dimensions)
• Model: microsoft/wavlm-base-plus-sv (Microsoft)
• Dimension: 512 (float32)
• Storage: vec_speakers virtual table (sqlite-vec)
• Pipeline Stage: speaker_embed (Level 4)
• Input: Audio waveform + transcript segments from database
WavLM (Wave Language Model) is a self-supervised speech representation model fine-tuned
for speaker verification. Its embeddings capture speaker-discriminative characteristics — vocal
timbre, fundamental frequency, formant structure — enabling speaker diarization through
clustering.
Processing Pipeline:
1. Audio is loaded and resampled to 16kHz mono.
2. Voice Activity Detection: Silero VAD identifies speech segments, filtering out silence
and background noise. If Silero is unavailable, an energy-threshold detector provides a
fallback.
ClipCannon Technical White Paper v0.1.0
Page 9
3. Each speech segment is processed through WavLM. The last hidden state is meanpooled across time, L2-normalized, and truncated/padded to exactly 512 dimensions.
4. Speaker Clustering: Agglomerative clustering with distance_threshold=0.7,
metric="cosine", linkage="average" groups segments by speaker identity.
5. Speaker records are created in the speakers table with labels (speaker_0, speaker_1,
...), total speaking time, and speaking percentage.
6. Transcript segments and words are updated with assigned speaker_id values via maxoverlap matching between VAD speech segments and transcript time ranges.
7. Speaker embeddings are stored in vec_speakers for subsequent similarity queries.
Diarization Strategy: Rather than traditional diarization (which requires a known number of
speakers), ClipCannon uses agglomerative clustering with a cosine distance threshold, allowing
automatic discovery of an arbitrary number of speakers. The distance threshold of 0.7 was
tuned to balance between over-segmentation (splitting one speaker into multiples) and undersegmentation (merging different speakers).
Fallback: If WavLM fails entirely, all transcript segments are assigned to a single speaker_0 via
_insert_fallback_speaker(), ensuring downstream stages (chronemic analysis, highlight scoring)
can still execute.
4.5 Voice Identity Embeddings — Qwen3-TTS ECAPA-TDNN (2048
dimensions)
• Model: marksverdhei/Qwen3-Voice-Embedding-12Hz-1.7B (ECAPA-TDNN speaker
encoder)
• Dimension: 2048 (float32)
• Storage: voice_profiles table (in ~/.clipcannon/voice_profiles.db)
• Usage Context: Voice cloning verification (Phase 3) and Voice Agent identity matching
This is the highest-dimensional embedding in the system, designed specifically for speaker
verification in the context of voice cloning. The ECAPA-TDNN (Emphasized Channel Attention,
Propagation and Aggregation with Time Delay Neural Network) architecture is purpose-built for
extracting speaker-discriminative features.
Role in Voice Cloning:
1. Profile Creation: When a voice profile is created, the system computes a 2048-
dimensional reference embedding from the speaker’s training audio clips. This
embedding is stored as a BLOB in the voice_profiles table.
2. Synthesis Verification: During voice synthesis (clipcannon_speak), each generated
audio candidate is embedded using the same ECAPA-TDNN model. The cosine
similarity between the candidate’s embedding and the reference profile embedding
determines identity fidelity.
3. Multi-Gate Verification: The voice verification pipeline applies three sequential gates:
Sanity Gate (duration check, clipping detection, SNR measurement, silence ratio),
Intelligibility Gate (Word Error Rate via Whisper transcription, punctuation-normalized
comparison), and Identity Gate (Speaker Embedding Cosine Similarity against the
reference embedding, with a configurable threshold of default 0.80).
ClipCannon Technical White Paper v0.1.0
Page 10
4. SECS-Optimized Synthesis: The clipcannon_speak_optimized tool generates N
candidates (default 8), scores each by SECS against the reference, and returns the best
match.
Relationship to Pipeline Embeddings: While the four pipeline embedding spaces (visual,
semantic, emotion, speaker) operate on video content during analysis, the voice identity
embedding operates on synthesized audio during production. The pipeline’s WavLM speaker
embeddings (512-dim) identify who is speaking in the source video; the ECAPA-TDNN
embeddings (2048-dim) verify that synthesized speech sounds like the target speaker. These
are complementary but distinct tasks — diarization vs. verification — hence the different models
and dimensionalities.
4.6 Supporting Models (Non-Embedding)
Beyond the five embedding models, ClipCannon employs additional ML models that do not
produce stored embeddings but are critical to the pipeline:
Model Purpose Stage
HTDemucs v4 (Meta) Audio source separation into
vocals/music/drums/bass stems
source_separation
WhisperX + wav2vec2
alignment
Speech-to-text with 20-50ms word-level
precision
transcribe
PaddleOCR PP-OCRv5 On-screen text detection ocr
pyiqa BRISQUE Frame quality assessment quality
SenseVoice Small
(FunASR)
Reaction detection (laughter, applause) reactions
Qwen3-8B (Qwen) Narrative structure analysis narrative_llm
Qwen3-TTS 1.7B (Qwen) Voice synthesis for video production speak tool
Qwen3-TTS 0.6B (fasterqwen3-tts)
Real-time voice synthesis for Voice
Agent
Voice Agent TTS
ACE-Step v1.5 AI music generation via diffusion generate_music tool
LatentSync 1.6
(ByteDance)
Lip-sync video diffusion lip_sync tool
Silero VAD Voice activity detection speaker_embed,
Voice Agent
OpenCV Haar /
MediaPipe
Face detection for scene analysis and
smart crop
scene_analysis,
smart_crop
ClipCannon Technical White Paper v0.1.0
Page 11
openWakeWord "Hey Jarvis" wake word detection Voice Agent
activation
5. Vector Storage and Retrieval
ClipCannon uses sqlite-vec, a SQLite extension that provides vec0 virtual tables for
approximate nearest-neighbor (ANN) search. Four vector tables are created per project:
Table Dimension Source Model Content
vec_frames float[1152] SigLIP-SO400M One embedding per
extracted frame
vec_semantic float[768] Nomic Embed v1.5 One embedding per
transcript segment
vec_emotion float[1024] Wav2Vec2 One embedding per 5-
second audio window
vec_speakers float[512] WavLM One embedding per speech
segment
5.1 Query Patterns
The primary query interface is the clipcannon_search_content MCP tool, which supports two
modes:
7. Semantic Search: The query text is embedded using Nomic Embed (with
"search_query: " prefix) and compared against vec_semantic using sqlite-vec’s KNN
operator. This returns transcript segments ranked by cosine similarity to the query.
8. Text Match Fallback: If sqlite-vec is unavailable (the extension is optional), the system
falls back to SQL LIKE queries against the transcript_segments.text column.
5.2 Cross-Modal Retrieval
While the vector tables are queried independently, the results are joined with temporal metadata
to enable cross-modal analysis. For example, the clipcannon_get_frame tool returns not just the
frame image but also the concurrent transcript text, speaker identity, emotion values, topic, shot
type, quality score, pacing label, OCR text, and profanity status — all retrieved by joining
against the frame’s timestamp across multiple tables.
6. Cross-Stream Intelligence
The Discovery Engine (src/clipcannon/tools/discovery.py) implements four MCP tools that
synthesize signals across all embedding spaces and structured analysis results:
ClipCannon Technical White Paper v0.1.0
Page 12
6.1 Find Best Moments
clipcannon_find_best_moments identifies key moments using purpose-aware scoring. The
scoring weights shift based on the intended purpose:
Purpose Primary Signal Secondary Signals
hook Emotion energy, visual variety Reactions, pacing
highlight Emotion energy, reactions Semantic density, quality
cta Speaker confidence, pacing Semantic density
tutorial_step Semantic density, pacing OCR presence, quality
Each moment candidate is scored across all available streams, with weights adjusted by
purpose. This is where the embedding architecture provides its greatest value: moments that
score highly across multiple modalities (high emotion + audience reaction + topic shift + scene
boundary) are ranked above moments that are strong in only one dimension.
6.2 Find Cut Points
clipcannon_find_cut_points detects natural edit boundaries by computing convergence
scores across four signal types:
9. Silence boundaries — from silence_gaps table (acoustic analysis)
10. Beat boundaries — from beats table (beat detection via librosa/scipy)
11. Scene boundaries — from scenes table (visual embedding similarity drops)
12. Sentence boundaries — from transcript_segments table (end of sentences)
Points where multiple signal types converge (e.g., a silence gap that coincides with a scene
boundary and sentence end) receive the highest scores. This approach produces cuts that feel
natural across all perceptual channels rather than cutting mid-sentence or mid-beat.
6.3 Narrative Flow Analysis
clipcannon_get_narrative_flow validates the narrative coherence of a proposed edit before
rendering. Given a sequence of segments, it analyzes:
• Temporal ordering and gap detection
• Topic continuity (via semantic embeddings)
• Speaker consistency
• Emotional arc coherence
• Story beat alignment (from Qwen3-8B narrative analysis)
This tool enables the AI assistant to evaluate whether a proposed edit tells a coherent story
before committing to the computationally expensive rendering step.
6.4 Audio-Safe Cut Points
ClipCannon Technical White Paper v0.1.0
Page 13
clipcannon_find_safe_cuts identifies cut points where all audio signals are quiet or
transitioning, avoiding cuts that produce jarring audio discontinuities.
6.5 Highlight Scoring Algorithm
The highlights pipeline stage (Level 6) produces the system’s most comprehensive crossstream synthesis. It scores 30-second candidate windows using seven weighted signals:
Signal Weight Source Computation
Emotion energy 0.25 emotion_curve
(Wav2Vec2)
Average energy of overlapping
windows
Reaction
presence
0.20 reactions (SenseVoice) Binary: 1.0 if any reaction
overlaps
Semantic density 0.20 topics (Nomic Embed) Average topic coherence in
window
Narrative 0.15 transcript_segments Sentence boundary alignment
score
Visual variety 0.10 scenes (SigLIP) Scene change frequency vs.
expected
Quality 0.05 scenes (BRISQUE) Average frame quality
Speaker
confidence
0.05 transcript_segments
(WavLM)
Single-speaker dominance score
The weighted sum produces a composite score that reflects the “interestingness” of each
moment from a multi-modal perspective. Highlights are classified by their dominant signal
(emotional peak, audience reaction, key topic, visual montage, etc.) and stored in the highlights
table for use by discovery tools and the AI assistant.
7. Narrative Analysis via Large Language Model
The narrative_llm pipeline stage runs Qwen3-8B (Qwen, 8 billion parameters, FP16) after
transcription to perform structural analysis of the video’s content. Unlike the embedding-based
analysis stages, this stage uses a generative language model to reason about narrative
structure.
Inputs: The full transcript text, concatenated from transcript_segments.
Outputs (JSON stored in narrative_analysis table):
• Story beats: Identified narrative turning points with timestamps
• Open loops: Questions or tensions raised but not yet resolved, creating audience
engagement hooks
ClipCannon Technical White Paper v0.1.0
Page 14
• Chapter boundaries: Natural thematic divisions of the content
• Key moments: The most narratively significant moments for editing
• Narrative summary: A condensed textual summary of the content
Implementation Detail: The model runs in a subprocess (HF_HUB_OFFLINE=1) for clean
GPU memory management. This prevents VRAM fragmentation from model co-residency and
ensures the 8B-parameter model can load on GPUs that may already have pipeline models
resident.
The narrative analysis results feed into the highlight scoring algorithm (narrative signal, weight
0.15) and the narrative flow validation tool, providing a higher-order structural understanding
that pure embedding similarity cannot capture.
8. Scene Analysis and Layout Computation
The scene_analysis pipeline stage provides content-aware spatial intelligence for the editing
engine.
8.1 Scene Boundary Detection
Using SSIM (Structural Similarity Index Measure) comparison between consecutive frames, the
system detects visual scene boundaries. A maximum scene duration of 8 seconds is enforced
to prevent excessively long scenes in static content.
8.2 Spatial Region Detection
For each scene, OpenCV-based analysis identifies:
• Face positions: Bounding boxes of detected faces (Haar cascades or MediaPipe)
• Webcam overlay regions: Small face-containing rectangles in typical webcam positions
(screen corners), common in screen-recording content
• Content areas: The main content region, excluding browser chrome (top 70px) and OS
taskbar (bottom 50px)
• Content type classification: code, slides, browser, terminal, or unknown — based on
detected text patterns and layout
8.3 Pre-Computed Canvas Regions
For each scene, the system pre-computes canvas regions for four standard layout types on a
1080x1920 (9:16) canvas:
Layout Description Speaker Region Content Region
A 30/70 vertical split 576px top 1344px bottom
B 40/60 vertical split 768px top 1152px bottom
C Picture-in-Picture 240px circle overlay at
(24, 140)
Full canvas
ClipCannon Technical White Paper v0.1.0
Page 15
D Full-screen face Full canvas N/A
These pre-computed regions are stored as JSON in the scene_map table’s
canvas_regions_json column, enabling the AI assistant to create edits with precise canvas
compositing without manual coordinate measurement.
9. The Editing Engine
The editing engine implements a declarative Edit Decision List (EDL) architecture, fully
described by Pydantic models.
9.1 EDL Structure
An EditDecisionList specifies:
• Segments: Time ranges from the source video, with speed adjustments (0.25x-4.0x)
and per-segment canvas overrides
• Captions: Adaptive caption chunking (max 3 words, speech-rate-aware, 4 ASS styles)
• Crop: Smart cropping with face tracking, split-screen, PIP, or canvas compositing
• Canvas: Multi-region compositing with z-index ordering, fit modes, opacity, and
animated zoom
• Audio: Source volume, background music mixing, speech-aware ducking, sound effects
• Transitions: 10 transition types mapped to FFmpeg xfade filters
• Overlays: Lower thirds, title cards, logos, watermarks, CTAs with animation
• Color: Brightness, contrast, saturation, gamma, hue shift (global or per-segment)
• Motion: Zoom, pan, Ken Burns effects with configurable easing
9.2 Iterative Editing Workflow
ClipCannon is designed for iterative refinement, not one-shot generation:
13. Create: clipcannon_create_edit builds an EDL from segment specifications
14. Modify: clipcannon_modify_edit applies deep-merge changes with automatic version
history
15. Feedback: clipcannon_apply_feedback parses natural language (e.g., “make it shorter”,
“cut the intro”) into structured EDL changes
16. Branch: clipcannon_branch_edit forks platform-specific variants (e.g., TikTok ->
Instagram)
17. Revert: clipcannon_revert_edit restores previous versions from the version chain
18. Preview: clipcannon_preview_clip renders 2-5 second 540p previews without
consuming credits
Every modification auto-saves the previous state to the edit_versions table, creating a complete
version history chain.
9.3 Caption Generation
ClipCannon Technical White Paper v0.1.0
Page 16
The caption engine performs adaptive chunking of transcript words:
• Maximum 3 words per caption chunk
• Breaks at sentence punctuation and commas
• Adapts to speech rate: fast speech (>200 WPM) reduces words per chunk; slow speech
(<80 WPM) increases them
• Merges chunks separated by gaps < 200ms
• Generates ASS subtitle files with four visual styles: bold centered, word highlight, subtitle
bar, and karaoke
10. The Rendering Engine
The rendering engine translates EDL specifications into platform-optimized video files through
an async FFmpeg pipeline.
10.1 Pipeline Steps
19. Source validation: SHA-256 verification, generation-loss prevention (blocks re-rendering
of rendered output)
20. Profile resolution: One of 7 encoding profiles selected by target platform
21. Caption writing: ASS subtitle file generation
22. Crop computation: Face-tracking crop regions, smoothed via EMA across scenes
23. FFmpeg execution: Async subprocess with dynamic filter graph construction
24. Thumbnail generation: JPEG frame extraction at the optimal timestamp
25. Provenance recording: Hash chain linking the output to its source
26. Database update: Render metadata stored in the renders table
10.2 Encoding Profiles
Profile Resolution Bitrate Aspect Use Case
tiktok_vertical 1080x1920 8 Mbps 9:16 TikTok
instagram_reels 1080x1920 6 Mbps 9:16 Instagram Reels
youtube_shorts 1080x1920 8 Mbps 9:16 YouTube Shorts
youtube_standard 1920x1080 12 Mbps 16:9 YouTube long-form
youtube_4k 3840x2160 40 Mbps 16:9 YouTube 4K
facebook 1080x1920 6 Mbps 9:16 Facebook Reels
linkedin 1080x1080 5 Mbps 1:1 LinkedIn
ClipCannon Technical White Paper v0.1.0
Page 17
All profiles use h264_nvenc (NVENC hardware encoder) with automatic fallback to libx264
software encoding.
10.3 Segment-Level Caching
The rendering engine computes a deterministic content hash for each segment based on the
source SHA-256, segment specification, encoding profile, canvas configuration, color settings,
and overlay specifications. Previously rendered segments with identical hashes are served from
cache, dramatically reducing re-render time during iterative editing.
11. The Audio Engine
The audio engine provides three tiers of audio generation plus mixing and cleanup:
Tier 1: AI Music Generation (ACE-Step v1.5)
GPU-accelerated music diffusion model generating original music from text prompts. Requires
4+ GB VRAM; uses CPU offload on GPUs with < 8 GB. Produces WAV at 44.1 kHz.
Tier 2: MIDI Composition (FluidSynth)
Six preset chord progressions (ambient pad, upbeat pop, corporate, dramatic, minimal piano,
intro jingle) with theory-correct harmonies, multi-track MIDI generation (chords, melody, optional
bass, optional drums), and FluidSynth SoundFont rendering.
Tier 3: DSP Sound Effects (NumPy/SciPy)
Nine programmatic sound effects (whoosh, riser, downer, impact, chime, tick, bass drop,
shimmer, stinger) generated via mathematical waveform synthesis. Zero GPU usage.
Audio Mixing
Speech-aware mixing with automatic ducking: RMS energy analysis in 50ms windows identifies
speech regions, and background music is reduced by 6 dB with 200ms attack / 300ms release
ramps during speech.
12. The Voice Engine
The voice engine implements a complete voice cloning pipeline:
12.1 Data Preparation
clipcannon_prepare_voice_data extracts vocal stems from ingested projects, splits audio at
silence boundaries, matches segments with transcript words, performs phonemization, and
produces train/validation manifests.
12.2 Voice Profile Management
Voice profiles store:
ClipCannon Technical White Paper v0.1.0
Page 18
• Profile metadata (name, model path, training status)
• 2048-dimensional ECAPA-TDNN reference embedding (the speaker’s voice
fingerprint)
• Verification threshold (default 0.80 cosine similarity)
• Training data provenance (source project IDs, training hours)
12.3 Synthesis Pipeline
The clipcannon_speak tool uses Qwen3-TTS 1.7B for high-quality voice synthesis with an
iterative verification loop:
27. Generate: Qwen3-TTS produces a speech candidate using Full ICL (In-Context
Learning) mode with reference audio
28. Verify: Three-gate verification: Sanity (duration bounds, clipping check, SNR
measurement, silence ratio), Intelligibility (WER via Whisper transcription, punctuationstripped comparison), and Identity (SECS against reference 2048-dim embedding)
29. Retry or Accept: If any gate fails, regenerate (up to max_attempts, default 5)
30. Enhance: Post-process via Resemble Enhance (denoise + latent flow matching
upsampling from 24kHz to 44.1kHz broadcast quality)
12.4 Optimized Synthesis
clipcannon_speak_optimized generates N candidates (default 8), scores each by SECS, and
returns the best match. This approach maximizes voice identity fidelity at the cost of N times the
inference compute.
12.5 Multi-Voice Conversation
MultiVoiceSynth enables instant voice swapping for multi-speaker conversation generation.
Voice prompts are loaded once per session, and segments are concatenated with configurable
inter-speaker pauses.
13. The Avatar Engine
The avatar engine uses LatentSync 1.6 (ByteDance), a diffusion-based lip-synchronization
model, to generate talking-head videos from audio input and a driver video.
Architecture: VAE encoder/decoder + UNet3D + DDIM scheduler
Output Resolution: 512x512
Configurable: Inference steps (default 20), random seed
The clipcannon_generate_video tool chains voice synthesis and lip-sync into a single end-toend pipeline: text → Qwen3-TTS → LatentSync → lip-synced video.
14. The Voice Agent
ClipCannon Technical White Paper v0.1.0
Page 19
The Voice Agent (“Jarvis”) is a standalone real-time conversational AI system with on-demand
GPU lifecycle management.
14.1 Architecture
Activation (CPU only)
 Wake Word ("Hey Jarvis") or Hotkey (Ctrl+Space)
 |
 GPU Loading (~10-20s)
 |
Audio Pipeline (GPU, ~30 GB VRAM)
 Mic -> Silero VAD -> faster-whisper (Large v3)
 -> Qwen3-14B FP8 (local LLM)
 -> faster-qwen3-tts (0.6B, CUDA graphs)
 -> Speaker
14.2 Lifecycle States
State GPU Usage Trigger
DORMANT Zero (CPU-only wake word) System startup / dismiss phrase
LOADING Growing (~10-20s) Wake word or hotkey detected
ACTIVE Full (~30 GB) All models loaded
UNLOADING Shrinking "Go to sleep", "goodbye", "dismiss"
14.3 TTS Model Selection
The Voice Agent uses a different TTS model than ClipCannon’s voice engine:
Component Voice Agent ClipCannon
Model faster-qwen3-tts 0.6B Qwen3-TTS 1.7B
TTFB ~500ms ~2-5s
Quality Good (real-time optimized) Broadcast (verification loop)
CUDA Graphs Yes (4-5x speedup) No
Use Case Live conversation Video voiceover production
The 0.6B model achieves ~500ms time-to-first-byte through CUDA graph capture, making it
suitable for interactive conversation where latency is critical. The 1.7B model produces higherfidelity output suitable for video production, where quality justifies the additional inference time.
14.4 GPU Resource Sharing
ClipCannon Technical White Paper v0.1.0
Page 20
The Voice Agent manages VRAM contention with ClipCannon through process-level signal
control:
• Activation: pause_gpu_workers() sends SIGSTOP to ClipCannon GPU processes
• Deactivation: resume_gpu_workers() sends SIGCONT to restore them
• Memory cap: torch.cuda.set_per_process_memory_fraction(0.93) prevents OOM
thrashing
14.5 VRAM Budget (RTX 5090 32 GB target)
Component Model VRAM
ASR Whisper Large v3 float32 ~6 GB
LLM Qwen3-14B FP8 (vLLM/Ollama) ~15 GB
TTS faster-qwen3-tts 0.6B ~4 GB
Overhead KV caches + activations ~5 GB
Total ~30 GB
15. Provenance and Data Integrity
ClipCannon implements a tamper-evident hash chain that records every pipeline operation,
linking each step to its predecessor through cryptographic hashing.
15.1 Chain Hash Formula
chain_hash = SHA256(
 "{parent_hash}|{input_sha256}|{output_sha256}|{operation}
 |{model_name}|{model_version}
 |{json.dumps(model_params, sort_keys=True)}"
)
The first record in a project uses the sentinel GENESIS_HASH = "GENESIS" as the parent hash.
Each subsequent record incorporates the parent’s chain hash, creating a linked chain where
modifying any historical record invalidates all downstream hashes.
15.2 Provenance Records
Each record captures 22 fields including input/output file paths and SHA-256 hashes, model
metadata (name, version, quantization, parameters), execution metrics (duration, GPU device,
peak VRAM), and the computed chain hash.
15.3 Verification
verify_chain() walks the entire chain in chronological order, recomputing each record’s chain
hash and comparing it against the stored value. Any discrepancy identifies the exact record
where tampering or corruption occurred.
ClipCannon Technical White Paper v0.1.0
Page 21
15.4 Finalize-Stage Verification
The pipeline’s finalize stage runs chain verification as its penultimate step. If verification fails,
the project is marked as "error" rather than "ready", preventing use of potentially corrupted
analysis data.
16. Billing and Credit System
ClipCannon uses a credit-based billing model with HMAC-signed balances for tamper
resistance.
16.1 Credit Rates
Operation Credits
Analyze (ingest) 10
Render 2
Metadata generation 1
Publish 1
16.2 HMAC Integrity
Balance values are signed with HMAC-SHA256 using a machine-derived key:
machine_id = SHA256("{hostname}|{mac_addr}|{cpu_arch}")[:32]
hmac_key = SHA256("clipcannon-v1|{machine_id}")
signature = HMAC-SHA256(hmac_key, "balance:{balance}")
Every balance read verifies the HMAC signature via constant-time comparison. A signature
mismatch triggers a CRITICAL log and raises a fatal BillingError.
16.3 Credit Packages
Package Credits Price Per-Credit
Starter 50 $5 $0.10
Creator 250 $20 $0.08
Pro 1,000 $60 $0.06
Studio 5,000 $200 $0.04
ClipCannon Technical White Paper v0.1.0
Page 22
17. GPU Management and Resource Optimization
17.1 Precision Auto-Detection
The system automatically selects the optimal quantization precision based on GPU compute
capability:
Compute Capability Architecture Precision
12.0 Blackwell nvfp4
8.9 Ada Lovelace int8
8.6 Ampere int8
7.5 Turing fp16
No GPU CPU fallback fp32
17.2 Model Manager
The ModelManager uses an LRU (Least Recently Used) eviction strategy:
• > 16 GB VRAM: Concurrent mode — multiple models stay loaded; LRU eviction frees
space when needed
• <= 16 GB VRAM: Sequential mode — all models unloaded before loading a new one
17.3 Per-Stage Memory Pattern
Individual pipeline stages manage their own GPU memory lifecycle: import → load model →
inference → delete model → torch.cuda.empty_cache(). This prevents VRAM fragmentation
across the 22-stage pipeline.
18. MCP Protocol Integration
ClipCannon exposes 51 tools via the Model Context Protocol:
Category Tools Description
Project 5 Create, open, list, status, delete projects
Understanding 4 Ingest, transcript, frame retrieval, semantic search
Discovery 4 Best moments, cut points, narrative flow, safe cuts
Editing 11 Full EDL lifecycle with iterative refinement
ClipCannon Technical White Paper v0.1.0
Page 23
Rendering 8 Platform rendering, preview, inspection, scene map
Audio 4 AI music, MIDI, SFX, cleanup
Voice 4 Data prep, profiles, synthesis, optimized synthesis
Avatar 1 Lip-sync video generation
Video Generation 1 End-to-end text-to-video
Billing 4 Balance, history, estimates, spending limits
Disk 2 Usage analysis, cleanup
Config 3 Get, set, list configuration
The MCP server runs on stdio transport by default, with all tool results serialized as JSON
TextContent. Image-returning tools also provide ImageContent with base64-encoded inline
data.
19. Database Architecture
19.1 Per-Project Database
Each project maintains its own SQLite database at
~/.clipcannon/projects/{project_id}/analysis.db with:
• 31 core tables covering project metadata, transcripts, visual analysis, audio analysis,
scoring, content safety, pipeline tracking, scene analysis, narrative analysis, editing,
rendering, and audio assets
• 4 vector tables (sqlite-vec vec0): vec_frames, vec_semantic, vec_emotion,
vec_speakers
• 19 indexes optimized for time-range queries, status filtering, and version history
traversal
19.2 Schema Versioning
The database uses a three-version migration chain:
• v1: Phase 1 core tables (22 tables)
• v2: Phase 2 editing/rendering tables (6 tables)
• v3: Phase 3 voice profiles table (1 table)
New projects include all versions from creation; existing projects are migrated idempotently.
19.3 Connection Configuration
All connections apply: WAL journal mode, NORMAL synchronous, 64 MB page cache, foreign
keys ON, temp store in memory.
ClipCannon Technical White Paper v0.1.0
Page 24
19.4 Separate Databases
Database Location Purpose
Project DB ~/.clipcannon/projects/*/analysis.db All analysis, editing, rendering
data
License DB ~/.clipcannon/license.db Credit balances and
transactions
Voice Profiles DB ~/.clipcannon/voice_profiles.db Voice profiles with 2048-dim
embeddings
Voice Agent DB ~/.voiceagent/agent.db Conversation logs and latency
metrics
20. Testing and Verification
ClipCannon maintains 626 tests across 43 test files:
• 425 ClipCannon tests (24 files): Pipeline stages, billing, provenance, editing, rendering,
audio, discovery, voice, avatar
• 201 Voice Agent tests (19 files): ASR, TTS, LLM, conversation, transport, activation,
database
Additionally, 10 FSV (Full State Verification) scripts provide forensic-level testing with 750+
individual checks, verifying database state, module imports, edge cases, and integration
correctness.
21. Graceful Degradation
The system implements five layers of degradation:
31. Stage-level: Required stages abort the pipeline; optional stages log failures and
continue
32. Audio resolution: vocals.wav → audio_16k.wav fallback chain
33. Model-level: WhisperX → faster-whisper, pyiqa → Laplacian, Wav2Vec2 → RMS,
WavLM → single-speaker fallback, SenseVoice → spectral analysis, librosa → scipy,
NVENC → libx264
34. Extension-level: sqlite-vec unavailable → SQL LIKE search fallback
35. Finalize reconciliation: All failed/skipped streams documented in degradation note;
project still usable for editing
22. Conclusion
ClipCannon Technical White Paper v0.1.0
Page 25
ClipCannon demonstrates that multi-modal embedding architectures can transform video
understanding from a manual, intuition-driven process into a programmatic, AI-assisted
workflow. By maintaining five distinct embedding spaces — visual (1152-dim SigLIP), semantic
(768-dim Nomic Embed), emotional (1024-dim Wav2Vec2), speaker identity (512-dim WavLM),
and voice verification (2048-dim ECAPA-TDNN) — the system captures the full perceptual
richness of video content and makes it queryable through a unified database interface.
The cross-stream intelligence layer — highlight scoring, convergence-based cut detection,
narrative flow analysis — demonstrates that the combination of multiple specialized embeddings
produces insights that no single modality can provide. A “best moment” is not just a visual
highlight or an emotional peak; it is a convergence of signals across all channels, weighted by
the intended editorial purpose.
The system’s architecture — local-first execution, tamper-evident provenance, credit-based
billing, graceful degradation, and MCP protocol integration — positions ClipCannon as a
production-grade tool for AI-assisted video production. The addition of voice cloning with
ECAPA-TDNN verification and real-time conversational AI via the Voice Agent extends the
platform from passive video analysis to active content generation, creating a complete pipeline
from raw footage to lip-synced, voice-cloned, platform-optimized output.
Appendix A: Model Summary Table
# Model Provider Params Purpose Stage/Tool GPU
1 SigLIPSO400Mpatch14-384
Google ~400M Visual
embeddings
+ shot
classification
visual_embed,
shot_type
Yes
2 Nomic Embed
Text v1.5
Nomic AI ~137M Semantic
text
embeddings
+ topic
clustering
semantic_embed Yes
3 Wav2Vec2-
large-960h
Meta ~317M Emotion
embeddings
from audio
emotion_embed Yes
4 WavLM-baseplus-sv
Microsoft ~94M Speaker
embeddings
+ diarization
speaker_embed Yes
5 Qwen3-VoiceEmbedding
ECAPA-TDNN
Qwen ~1.7B Voice
identity
verification
(2048-dim)
Voice verification Yes
ClipCannon Technical White Paper v0.1.0
Page 26
6 HTDemucs v4 Meta ~83M Audio
source
separation
source_separation Yes
7 WhisperX /
faster-whisper
(Large v3)
OpenAI ~1.5B Speech-totext with
word
alignment
transcribe Yes
8 PaddleOCR PPOCRv5
PaddlePaddle ~12M On-screen
text
detection
ocr Optional
9 SenseVoice
Small
FunASR ~234M Reaction
detection
(laughter,
applause)
reactions Yes
10 Qwen3-8B Qwen 8B Narrative
structure
analysis
narrative_llm Yes
11 Qwen3-TTS
1.7B
Qwen 1.7B High-quality
voice
synthesis
speak tool Yes
12 faster-qwen3-tts
0.6B
Qwen 0.6B Real-time
voice
synthesis
Voice Agent Yes
13 ACE-Step v1.5 ACE ~1B AI music
generation
(diffusion)
generate_music Yes
14 LatentSync 1.6 ByteDance ~500M Lip-sync
video
diffusion
lip_sync Yes
15 Silero VAD Silero ~2M Voice
activity
detection
Multiple No
(CPU)
16 Qwen3-14B
FP8
Qwen 14B Local LLM
reasoning
(Voice
Agent)
Voice Agent brain Yes
17 openWakeWord David Scripka ~5M "Hey Jarvis"
wake word
detection
Voice Agent
activation
No
(CPU)
ClipCannon Technical White Paper v0.1.0
Page 27
Appendix B: Embedding Dimension Reference
Embedding
Space
Dimension Model Storage Table Data Type Normalization
Visual 1152 SigLIP-SO400M vec_frames float32 L2-normalized
Semantic 768 Nomic Embed
v1.5
vec_semantic float32 L2-normalized
Emotion 1024 Wav2Vec2-
large
vec_emotion float32 Mean-pooled
Speaker 512 WavLM-baseplus-sv
vec_speakers float32 L2-normalized
Voice Identity 2048 ECAPA-TDNN
(Qwen3)
voice_profiles
(BLOB)
float32 L2-normalized
Total embedding dimensionality per video: 1152 + 768 + 1024 + 512 = 3,456 dimensions
across four vector spaces for video content analysis, plus 2048 dimensions for voice identity
verification.
ClipCannon is open-source software licensed under MIT. For more information, visit the project
repository.

Derived Data Abundance: How Multi-Modal Embedding
Decomposition Solves the AI Training Data Crisis
A Research Paper on Teleological Constellation Training and the Data
Scaling Wall
Chris Royse April 2026
Abstract
The AI industry faces an imminent training data crisis. Research from Epoch AI
projects that the stock of usable public human-generated text — approximately 300
trillion tokens — will be exhausted by frontier models between 2026 and 2032, with
aggressive overtraining pushing this timeline even earlier. This paper argues that the
crisis is misframed: the bottleneck is not raw data volume but labeled, meaningful,
multi-dimensional training data. I present evidence from two implemented systems:
ClipCannon, a video understanding and avatar generation pipeline, and Context
Graph, a Rust-based semantic memory retrieval system. ClipCannon decomposes
video through 7 embedding modalities spanning 4,044 dimensions — from a single
16-minute video, its 23-stage analysis pipeline extracted over 12,000 individually labeled data points, sufficient to train a generative model for identity-locked avatar
synthesis. Context Graph applies the same principle to pure text, decomposing every
stored memory through 13 independent embedding models — semantic, temporal,
causal, lexical, code-structural, graph-relational, entity-level, and more — producing
13 independent representations plus 78 cross-correlations per input, a data multiplication factor of approximately 91× in meaningful training signals. I introduce the
concept of Derived Data Abundance — the principle that running existing data
through N independent embedding models produces N-dimensional labeled training
samples at a rate multiplicative to the original data volume — and argue that this
approach avoids the model collapse problem inherent in synthetic data generation
because the derived data is grounded in real observations, not recursive model outputs. I further argue that this constitutes a new category of compression — meaning
compression — that is fundamentally more valuable than the weight compression,
activation compression, and data compression techniques that dominate current AI
research (including Google’s TurboQuant, announced March 2026). Where existing
compression reduces bits per unit of information, meaning compression increases
meaningful signal per unit of raw data, directly addressing the training data wall
that threatens to stall the scaling paradigm. I propose that Teleological Constellation
Training (TCT), the method that emerged from this work, represents a generalizable
framework for transforming the existing stock of human-generated data into vastly
larger, richer, and automatically labeled training corpora suitable for next-generation
model training.
1
1. Introduction: The Data Wall
1.1 The Scaling Laws and Their Limits
The progress of artificial intelligence over the past decade has been driven by a remarkably simple observation: larger models trained on more data with more compute
produce better results. The scaling laws documented by Kaplan et al. (2020) and refined by Hoffmann et al. (2022) formalized this relationship, showing that model performance improves predictably as training compute, model parameters, and dataset
size increase in proportion. These laws underpin the tens of billions of dollars currently being invested in AI infrastructure worldwide.
But the scaling paradigm is approaching a structural constraint. While compute continues to scale through hardware improvements and massive datacenter buildouts,
and model parameters can be increased indefinitely in principle, the third leg of the
scaling tripod — training data — is finite. The stock of public human-generated text
on the internet, after filtering for quality and deduplication, amounts to roughly 300
trillion tokens. At current growth rates in training dataset sizes, frontier models will
consume this entire stock between 2026 and 2032, with the timeline compressing
to as early as 2025–2027 under aggressive overtraining regimes like those used for
Meta’s Llama 3 (overtrained by a factor of 10×).
This is not a speculative concern. Reports from multiple AI labs indicate that pretraining scaling is already showing diminishing returns. TechCrunch reported in late
2024 that major AI investors, founders, and CEOs confirmed that the methods and
expectations used to increase model capabilities over the past five years are now yielding diminishing improvements. The HEC Paris analysis from 2025 noted a growing
consensus inside labs that simply adding more data and compute will not produce the
capabilities once promised. The data wall is real, and the industry is hitting it now.
1.2 The Synthetic Data Trap
The most commonly proposed solution to data scarcity is synthetic data generation:
using existing AI models to generate additional training data. This approach has
shown promise in specific applications — autonomous vehicle simulation, healthcare
privacy protection, and augmentation of rare-event datasets. Gartner forecasts that
by 2030, synthetic data will be more widely used for AI training than real-world
datasets.
However, synthetic data carries a fundamental risk: model collapse. The landmark
2024 Nature paper by Shumailov et al. demonstrated that AI models trained recursively on AI-generated data experience progressive quality degradation. The models gradually lose information from the tails of the original distribution — first rare
patterns disappear (early model collapse), then the output distribution converges toward a narrow, homogeneous mass that bears little resemblance to the original data
(late model collapse). This phenomenon was observed across multiple model architectures including large language models, variational autoencoders, and Gaussian
mixture models.
The model collapse problem is structural, not incidental. When a model generates syn2
thetic data, it necessarily loses information from the original distribution — the generation process compresses the distribution toward its modes, amplifying common
patterns and attenuating rare ones. Training subsequent models on this compressed
data amplifies the compression. Over iterations, the feedback loop produces outputs
that are increasingly bland, repetitive, and divorced from the richness of real humangenerated content. While techniques like data accumulation (mixing synthetic with
real data), watermarking, and verification filters can delay collapse, none eliminate
the fundamental information loss inherent in recursive generation.
1.3 The Thesis: A Third Path
This paper proposes that the data crisis admits a third resolution, distinct from both
“find more raw data” and “generate synthetic data.” The resolution is to derive more
meaningful, labeled data from the data we already have, using multi-modal embedding decomposition.
The key insight is this: a single piece of data — a video clip, an audio recording, a
document, an image — contains far more information than any single analysis can extract. Running the same data through multiple independent embedding models, each
trained to perceive a different dimension of meaning (visual appearance, semantic
content, emotional valence, speaker identity, prosody, sentiment), produces multiple
independent labeled representations of the same underlying observation. Each representation is grounded in real data, not generated by a model. Each carries a different
dimension of meaning that can serve as a training signal. And the combinatorial explosion of multi-modal labels — a single clip simultaneously labeled for visual content,
emotion, speaking style, topic, and identity — creates training data of a richness and
specificity that raw text or images cannot match.
I call this principle Derived Data Abundance: the observation that the effective
training data available from a fixed corpus grows multiplicatively with the number of
independent embedding models applied to it, because each model extracts a different
dimension of meaning that serves as an independent training signal.
I demonstrate this principle concretely through ClipCannon, a video understanding
system that extracts seven embedding modalities from source video, and Teleological
Constellation Training (TCT), the training method that emerged from this decomposition. From 16 minutes of interview video, ClipCannon produced over 12,000 individually labeled training samples across 4,044 embedding dimensions, sufficient to train a
generative video model that produces identity-locked avatar video indistinguishable
from the original subject.
2. Background: The Training Data Landscape
2.1 The Stock of Human-Generated Data
The Epoch AI research group estimates the effective stock of quality-adjusted, deduplicated public human text at approximately 300 trillion tokens. This figure accounts
for the filtering that modern training pipelines apply: removing low-quality content,
3
deduplicating near-identical pages, and excluding machine-generated text. The raw
volume of internet content is vastly larger, but most of it is noise — spam, boilerplate,
machine translations, and increasingly, AI-generated content that pollutes the signal.
The visual data stock is harder to quantify but follows similar dynamics. While the
internet contains billions of images and millions of hours of video, most of this content
is unlabeled, unstructured, and of variable quality. The datasets that have powered
computer vision advances — ImageNet, LAION-5B, WebVid-10M — represent a small
fraction of total visual content, curated for quality and labeled (often noisily) for specific tasks.
The critical point is not the raw volume of data but the amount of labeled, meaningful
training signal available. Raw data is abundant; labeled data with multi-dimensional
meaning is scarce. This distinction is central to the argument that follows.
2.2 Current Approaches to Data Enrichment
The AI industry has responded to data scarcity through several strategies, each with
significant limitations.
Licensing deals represent the most straightforward approach: pay content creators
for access to high-quality human-generated content. News Corp signed a deal with
OpenAI exceeding $250 million over five years. Reddit negotiated with Google and
OpenAI for $203 million annually. These deals expand the available corpus but do
not change its fundamental nature — the data is still text, still one-dimensional in its
training signal, and still finite in volume.
Multi-modal pre-training attempts to leverage data from domains beyond text.
Models like GPT-4V and Gemini are trained on both image and text data, and research
has shown synergy between modalities. However, multi-modal pre-training typically
treats each modality as a separate input channel rather than extracting multiple embedding dimensions from the same data.
Synthetic data generation via generative models has shown promise for specific
applications but faces the model collapse constraint described above. The Truncated
Cross-Entropy loss function, verified reasoning filters, and data accumulation strategies can mitigate but not eliminate the fundamental problem.
Data efficiency improvements — better tokenization, improved architectures, curriculum learning — reduce the amount of data needed for a given performance level.
But the gains are incremental, not transformative. Undertraining (growing model parameters while holding dataset size constant) can deliver the equivalent of up to two
additional orders of magnitude of compute-optimal scaling, but eventually plateaus.
2.3 What’s Missing: Meaning Density
The common thread across all current approaches is that they treat data as a volume
problem: more tokens, more images, more modalities. But the real constraint is not
volume — it is meaning density. A billion tokens of low-quality web scrape contain
less training signal than a million tokens of expert-curated, richly annotated content.
4
The scaling laws depend not on raw data volume but on the information content of
the training corpus as perceived by the model.
This observation suggests a different strategy: instead of finding or generating more
data, extract more meaning from the data we already have. If a single video clip can
be decomposed into twelve independently meaningful representations — visual, semantic, emotional, prosodic, temporal, identity-related — then the effective training
corpus is twelve times larger in terms of meaningful signals, even though the raw
data hasn’t changed.
3. ClipCannon: A Case Study in Multi-Modal Decomposition
3.1 System Overview
ClipCannon is a video understanding, editing, and avatar generation system implemented as a Python package of 247 source files and approximately 67,585 lines of
code. It exposes 58 tools via the Model Context Protocol (MCP), backed by a 23-
stage analysis DAG (directed acyclic graph), a tamper-evident provenance chain, and
an HMAC-secured credit billing system. Two companion systems — Phoenix, a GPUnative avatar engine, and VoiceAgent, a real-time conversational agent — extend its
capabilities to identity-locked video generation and autonomous meeting participation.
For the purposes of this paper, ClipCannon’s most relevant capability is its analysis
pipeline: a topologically sorted graph of 23 processing stages that decomposes source
video into multiple independent embedding spaces, producing thousands of labeled
data points from a single input.
3.2 The 23-Stage Analysis DAG
When ClipCannon ingests a source video, it runs the following pipeline stages, each
producing a different dimension of analysis:
Required stages (failure aborts the pipeline): probe (format validation and metadata
extraction), VFR normalization (constant frame rate conversion), audio extraction,
frame extraction, transcription (WhisperX large-v3 with anti-hallucination filtering),
and finalize.
Optional stages (failure logged, pipeline continues): source separation (Demucs vocal/instrumental split), visual embedding (SigLIP 1,152-dim per frame), OCR, quality
assessment, shot type classification, storyboard generation, scene analysis (SSIMbased boundaries with face detection), semantic embedding (Nomic 768-dim per transcript segment), narrative analysis (Qwen3-8B story beats), prosody analysis (F0, energy, speaking rate per sentence), speaker embedding (WavLM 512-dim), emotion
embedding (Wav2Vec2 1,024-dim), reaction detection, acoustic analysis, beat detection, profanity filtering, chronemic analysis, and highlights scoring.
These 23 stages run concurrently where dependency allows (using topological sort
and asyncio.gather for level-parallel execution), and produce results into a per-project
5
SQLite database with 37 tables and 4 vector tables backed by sqlite-vec.
3.3 The Seven Embedding Modalities
From the pipeline’s output, seven independent embedding modalities are extracted,
spanning 4,044 dimensions:
1. Visual (SigLIP-SO400M-patch14-384, 1,152 dimensions): Captures facial appearance, pose, expression, lighting, composition — everything perceptually relevant about a video frame.
2. Semantic (Nomic-embed-text-v1.5, 768 dimensions): Captures the meaning of
spoken content — topics, vocabulary, argument structure, conceptual density.
3. Emotion (Wav2Vec2-large-emotion, 1,024 dimensions): Captures vocal emotion
— arousal, valence, dominance — from the raw audio signal, independent of the
words being spoken.
4. Speaker (WavLM-large, 512 dimensions): Captures voice identity — timbre,
formant structure, vocal tract characteristics — a biometric signature of the
speaker.
5. Prosody (Custom extraction, 12 dimensions): Captures speaking style — fundamental frequency (F0) contour, energy envelope, speaking rate, pitch variation,
pause ratios — the musical quality of speech.
6. Sentiment (all-MiniLM-L6-v2, 384 dimensions): Captures sentence-level sentiment and intent — whether a statement is assertive, questioning, concessive,
emphatic.
7. Voice (ECAPA-TDNN, 192 dimensions): Captures a speaker verification embedding — a compact signature used to determine whether two audio samples come
from the same person.
Each modality is extracted by a frozen, independently trained model. The models
were not trained together; they were developed by separate research groups for separate purposes. Their independence is what gives the decomposition its power: each
one captures a dimension of meaning that the others miss, and together they define a
multi-dimensional space in which data can be labeled with unprecedented specificity.
3.4 Data Multiplication in Practice: The Santa Dataset
The reference material for the system’s demonstration is a 975-second (16-minute,
15-second) interview video of a single subject (“Santa”). From this single video, ClipCannon’s pipeline produced the following labeled data:
Modality Samples Dimensions Total Parameters
Visual (SigLIP) 1,725 1,152 1,987,200
Semantic (Nomic) 200 768 153,600
Emotion (Wav2Vec2) 343 1,024 351,232
Speaker (WavLM) 300 512 153,600
6
Modality Samples Dimensions Total Parameters
Prosody (Custom) 188 12 2,256
Sentiment (MiniLM) 202 384 77,568
Voice (ECAPA-TDNN) 188 192 36,096
FLAME Expression 3,477 100 347,700
Total 3,109,252
Additionally, the curated training data pipeline produced 2,362 individual training
clips, each automatically labeled with:
• A speaking/breathing classification (derived from transcript word alignment and
prosody)
• An emotion tag (derived from the emotion embedding and Plutchik mapping)
• An energy level (derived from prosody analysis)
• Viseme sequences (15 MPEG-4 visemes derived from CMU pronunciation dictionary mapping, with 7,916 real samples)
• Scene boundaries and shot types (derived from visual analysis)
• Speaker diarization labels (derived from speaker embedding)
From 16 minutes of video, the system derived over 12,000 individually labeled,
multi-dimensionally annotated data points. Each data point carries meaning
across multiple embedding spaces simultaneously — a single video clip is simultaneously labeled for what it looks like, what it sounds like, what emotion it conveys,
what words are being said, how those words are being delivered, and who is speaking
them.
This is the core demonstration of Derived Data Abundance: the ratio of meaningful
labeled training samples to raw input data is not 1:1 but orders of magnitude greater,
because each embedding model extracts a different dimension of meaning from the
same underlying observation.
4. Why Derived Data Avoids Model Collapse
4.1 The Structural Difference
The model collapse problem, as characterized by Shumailov et al. (2024), arises from
a specific feedback loop: a generative model produces synthetic data, that data is
used to train a successor model, and the successor produces data that is used to
train the next generation. Each generation compresses the distribution toward its
modes, losing tail information irreversibly.
Derived data avoids this feedback loop entirely, for a structural reason: the data
is not generated by the model being trained. The embedding models (SigLIP,
WavLM, Wav2Vec2, Nomic, etc.) are frozen. They do not learn from the data they
process. They do not produce synthetic data points. They produce measurements —
vector representations of real observations that capture specific dimensions of mean7
ing. These measurements are as grounded in reality as the original data; they are
simply different projections of the same underlying signal.
The distinction is analogous to the difference between photocopying a document (generative reproduction, subject to cumulative degradation) and measuring its physical
properties with different instruments (analytical decomposition, each measurement
adding new information without degrading the original). The embedding models are
measurement instruments, not generators. Their outputs carry information about the
real data, not approximations of it.
4.2 Information Preservation
A key property of well-trained embedding models is that they are approximately
information-preserving for the dimensions they are designed to capture. A SigLIP
embedding of a face image captures almost everything perceptually relevant about
that face — identity, expression, pose, lighting. A WavLM embedding of a voice
sample captures almost everything relevant about that voice — timbre, formant
structure, speaker characteristics. The embeddings are lossy (they compress highdimensional inputs to fixed-dimensional vectors), but the loss is concentrated in
dimensions that are irrelevant to the model’s trained objective.
This means that the derived data preserves the richness and diversity of the original
data distribution, including its tails. A rare facial expression that appears only once
in 16 minutes of video will produce a rare visual embedding — but the embedding
faithfully represents that rare expression, it does not compress it toward the mean. A
speaker’s unusual prosodic pattern on a single sentence will produce a rare prosody
vector — but the vector faithfully represents that pattern.
In contrast, synthetic data generation necessarily smooths over these rarities. A generative model trained on 16 minutes of video will produce outputs that cluster around
the most common expressions, voices, and styles. The rare tail events that define individuality and provide the richest training signal are precisely what gets lost.
4.3 Multi-Modal Cross-Validation
The use of multiple independent embedding models provides an additional safeguard
against information loss: cross-modal validation. If the visual embedding suggests a
happy expression but the emotion embedding suggests sadness, the inconsistency is
detectable. If the semantic embedding suggests a question but the prosody embedding suggests a statement, the inconsistency is detectable. Each modality serves as
a check on the others.
This cross-validation property is analogous to multi-factor authentication in security.
A single embedding model might misrepresent a data point due to noise, domain shift,
or model limitation. But the probability that N independent models all misrepresent
the same data point in a consistent way decreases exponentially with N. With seven
modalities, the effective false-acceptance rate is vanishingly small.
8
5. Teleological Constellation Training: The Method
5.1 From Decomposition to Training
Teleological Constellation Training (TCT) is the training method that emerged from
ClipCannon’s multi-modal decomposition. It uses the derived data not merely as training inputs but as the definition of the training target: the model’s goal is to produce
outputs that, when re-embedded through the same frozen models used to decompose
the reference data, fall within a configurable distance of the reference centroids.
The method has three phases:
Phase 1: Constellation Construction. The reference data (e.g., 16 minutes of
video) is decomposed through the seven embedding models. For each modality,
all embeddings are L2-normalized, their mean is computed, and the mean is renormalized to unit length. This produces a centroid vector per modality — the
geometric center of the target identity in that embedding space. The set of all
centroids across all modalities is the constellation.
Phase 2: Constellation-Conditioned Training. A generative model (in our case,
EchoMimicV3 fine-tuned via LoRA) is trained with loss functions that incorporate distance from the constellation centroids. The model is not just minimizing reconstruction error — it is minimizing distance from the frozen identity definition. This gives
the training a teleological character: the model’s purpose is to orbit the constellation.
Phase 3: Runtime Constellation Guard. At inference time, every generated output is re-embedded through the frozen models and compared to the constellation
centroids via cosine similarity. Any output that falls below a per-modality threshold
is rejected. The guard provides a hard, verifiable guarantee that the model’s outputs
remain within the identity manifold — a guarantee that no scalar reward system can
match.
5.2 The Four-Level Expression Hierarchy
Beyond identity locking, TCT derives a behavioral vocabulary from the decomposed
data. This vocabulary is organized into four levels:
Level 0 — Action Units: 17 FLAME expression coefficient indices mapped to FACS
Action Units (Ekman & Friesen, 1978), providing a human-interpretable description
of facial muscle activations.
Level 1 — Micro-Expression Groups: 40 clusters discovered by K-Means over
FLAME expression coefficients, representing the subject’s personally distinctive facial configurations — not generic expressions, but their expressions.
Level 2 — Expression Skills: 34 named behavioral primitives (genuine_laugh,
warm_smile, thoughtful_pause, etc.) with temporal structure (onset/peak/offset
phases) and natural language prompts for conditioning the generative model.
Level 3 — Behavioral Constellations: 8 emotional states (warm_conversational,
emotional_recall, happy_storytelling, etc.), each expressed as a cycling sequence of
9
skills. These constellations provide high-level behavioral control: a single API call to
set the emotional state produces frame-by-frame micro-expression conditioning.
All four levels are derived from the decomposed data. No manual labeling is required.
The behavioral vocabulary is discovered automatically from the clustering of FLAME
parameters, and the skills and constellations are compositions of discovered groups.
This is Derived Data Abundance applied to behavioral control: the same data that
defined the identity also defined 8 controllable emotional states, 34 expression skills,
and 40 micro-expression groups.
5.3 Results
From 16 minutes of interview video, TCT produced a system capable of generating
avatar video with the following verified properties:
• Visual similarity to the target identity ≥ 0.70 cosine similarity (against a 1,152-
dimensional centroid computed from 1,725 reference frames)
• Speaker similarity ≥ 0.80 cosine similarity (against a 512-dimensional voice centroid)
• Voice clone quality ≥ 0.95 Speaker Encoder Cosine Similarity (SECS)
• 8 independently controllable emotional states
• 34 micro-expression skills with temporal phase control
• Real-time inference capability for meeting bot deployment
These results were achieved from a dataset that, measured in traditional terms, is
tiny: 16 minutes of video, approximately 2,400 training clips. The leverage comes
not from data volume but from data richness — the multi-modal decomposition that
turned 16 minutes into 12,000+ independently meaningful training signals across
4,044 dimensions.
6. Context Graph: Proving Universality Beyond Video
6.1 From Video to Text — The Same Principle
If Derived Data Abundance were limited to video — a medium that naturally combines
visual, auditory, and linguistic channels — it would be a useful but narrow technique.
To demonstrate that the principle is truly universal, I built a second system: Context
Graph, a Rust-based semantic memory retrieval system that applies the identical
multi-modal embedding decomposition approach to pure text.
Context Graph is a 370,000-line Rust codebase comprising 10 crates, exposing 58
MCP tools over a JSON-RPC 2.0 server. It runs on GPU (targeting NVIDIA RTX 5090
via the HuggingFace Candle framework) and stores all data in RocksDB with 51 column families. Its central insight is identical to ClipCannon’s: a single piece of text,
when embedded through multiple independent models, reveals multiple dimensions
of meaning that are invisible to any single model.
10
6.2 Thirteen Embedders for Text
Where ClipCannon uses 7 embedding modalities across 4,044 dimensions, Context
Graph uses 13 independent embedders spanning dense, sparse, temporal, causal,
entity, graph, code, and token-level representations:
ID Name Dimensions What It Captures
E1 Semantic 1,024D dense General meaning
(e5-large-v2)
E2 Temporal Recent 512D dense Recency via
sinusoidal
positional encoding
E3 Temporal Periodic 512D dense Cyclical patterns
via Fourier basis
(time-of-day,
day-of-week)
E4 Temporal
Positional
512D dense Sequence position
within a session
E5 Causal 768D × 2
(asymmetric)
Cause-effect
directionality
(nomic-embed
fine-tuned)
E6 Sparse Lexical ~30K sparse Exact keyword
matching
(SPLADE)
E7 Code 1,536D dense Code patterns and
structure (QodoEmbed-1-1.5B)
E8 Graph 1,024D × 2
(asymmetric)
Connectivity and
relationship
structure
E9 HDC 10,000-bit →
1,024D
Typo-tolerant
matching via
hyperdimensional
computing
E10 Contextual 768D × 2
(asymmetric)
Paraphrase and
contextual
equivalence
E11 Entity 768D Entity knowledge
via KEPLER
(RoBERTa +
TransE)
E12 Late Interaction 128D per token Token-level
ColBERT MaxSim
scoring
E13 SPLADE v3 ~30K sparse Learned keyword
expansion
11
Every piece of text stored in Context Graph simultaneously receives all 13 embeddings. The resulting structure — called a TeleologicalFingerprint (approximately
46 KB per memory) — preserves 100% of the information from all 13 spaces without
any fusion at storage time. Fusion occurs only at query time via Weighted Reciprocal
Rank Fusion (RRF), where different weight profiles can emphasize different combinations of embedders depending on the task.
6.3 What 13 Lenses Reveal That 1 Cannot
The power of the approach becomes concrete when you examine what each embedder
sees that the others miss:
Cross-embedder anomaly detection. The search_cross_embedder_anomalies tool
finds memories that score highly in one embedder but poorly in another. A memory
that is semantically similar (high E1) but causally unrelated (low E5) is a different kind
of relationship than one that is both semantically and causally similar. These “blind
spots” — things visible to one lens but invisible to another — are precisely the kind
of rich, nuanced labels that traditional single-embedder systems cannot produce.
Causal direction. E5 uses asymmetric embeddings: cause and effect vectors are
different, with a 1.2× cause-to-effect boost and 0.8× effect-to-cause dampening. A
single semantic embedder (E1) would rank “A causes B” and “B causes A” identically.
The causal embedder distinguishes them, adding a training signal that captures the
directionality of relationships — something invisible to symmetric embeddings.
Temporal structure. E2, E3, and E4 encode temporal information that is completely
absent from semantic content. Two memories with identical text but different timestamps produce different E2 vectors. Two memories from the same time of day but
different content produce similar E3 vectors. These temporal signals are independent
training dimensions that text-only models cannot access.
Entity knowledge. E11 (KEPLER) maps text into a knowledge graph embedding
space using TransE, where relationships between entities are represented as vector
translations (relation = tail − head). The infer_relationship tool uses this to reason about entity relationships purely through vector arithmetic — a capability that
requires entity-level understanding, not just semantic similarity.
Code structure. E7 (Qodo-Embed) understands code at a structural level — function
signatures, import patterns, API relationships — in 1,536 dimensions. Text that describes code (“implement a sort algorithm”) and the code itself land in nearby regions
of E7 space, even though they share almost no surface tokens.
6.4 Emergent Topic Discovery from Multi-Space Agreement
Context Graph’s clustering subsystem demonstrates a particularly powerful consequence of multi-embedder decomposition: emergent topic discovery through
cross-space agreement.
The system runs HDBSCAN clustering independently in each of the 13 embedding
spaces, then looks for memories that cluster together across multiple spaces simultaneously. When memories co-cluster in E1 (semantic), E7 (code), and E11 (entity)
12
but not in E5 (causal), that pattern defines a specific type of topic — one with shared
meaning, shared code patterns, and shared entities, but without causal relationships.
A different topic might co-cluster in E1 and E5 but not E7 — shared meaning with
causal relationships but no code involvement.
The topic detection threshold requires weighted agreement across at least 2.5 embedding spaces (with temporal embedders E2-E4 excluded from topic scoring). This
means topics emerge only when multiple independent lenses agree that a cluster
exists — providing a robustness guarantee that no single-embedder clustering can
match.
The cross-space agreement pattern produces a 13-dimensional topic profile — a
vector where each dimension represents how strongly a topic expresses in that embedding space. These profiles are themselves rich, automatically generated labels
that describe the multi-dimensional character of each discovered topic.
6.5 The Teleological Comparator: 78-Dimensional Cross-Correlation
Beyond independent per-space analysis, Context Graph computes the full pairwise
cross-correlation between all 13 embedders for each memory, producing C(13,2) =
78 cross-correlation features. Combined with 13 per-space alignment scores and 6
group-level aggregations, the system creates a TeleologicalVector that captures not
just what each embedder sees individually, but how the embedders’ views relate to
each other.
This cross-correlation structure is where the multiplicative data enrichment becomes
most apparent. From a single text input, the system derives 13 independent views,
78 pairwise relationships between those views, 6 group-level summaries, and an optional Tucker tensor decomposition of the full 13×13×1024 interaction tensor. The
information content of this representation is orders of magnitude richer than any single embedding — and it is all derived automatically from frozen models applied to
real data.
6.6 Implications for Universality
Context Graph demonstrates that the Derived Data Abundance principle applies
equally to text as to video, and that the number of useful embedding dimensions for a
single modality can be much larger than initially expected. Where one might assume
that text “only needs” a semantic embedder, Context Graph shows that 13 independent lenses — semantic, temporal, causal, lexical, code-structural, graph-relational,
entity-level, token-level, typo-robust, paraphrase-aware, and sparse keyword — each
capture genuinely independent dimensions of meaning that the others miss.
The system proves that multi-embedder decomposition is not a multimodal technique
— it is a universal data enrichment technique applicable to any data type for
which multiple independent embedding models exist. A single sentence of text, when
analyzed through 13 lenses, produces 13 independent labeled representations plus
78 cross-correlations — a data multiplication factor of approximately 91× in terms of
meaningful training signals, from a single input.
13
7. Generalizing the Principle: Derived Data Abundance at Scale
7.1 The Fundamental Argument
The argument for Derived Data Abundance as a solution to the AI training data crisis
rests on three observations:
Observation 1: Data is information-dense. A single video contains visual, auditory, linguistic, emotional, prosodic, temporal, and spatial information simultaneously. A single text document contains semantic, syntactic, stylistic, topical, and
pragmatic information. Current training approaches typically extract only one or
two of these dimensions, leaving the majority of the information unused.
Observation 2: Embedding models extract independent dimensions. Each
frozen embedding model is a learned measurement instrument that extracts a specific dimension of meaning from data. Running the same data through N independent
embedding models produces N independent training signals. The effective training
corpus grows multiplicatively with the number of models applied.
Observation 3: The labels are automatic and grounded. Unlike traditional data
labeling (which requires human annotators) or synthetic data generation (which requires generative models), embedding-based labeling is automatic (the frozen model
produces the label) and grounded (the label is a measurement of real data, not a
model’s approximation of what data might look like). This eliminates both the cost of
human annotation and the risk of model collapse.
7.2 Scaling the Approach
Consider the implications at internet scale. The web contains hundreds of millions of
hours of video, billions of images, and trillions of text documents. If each of these data
points were decomposed through even three or four independent embedding models,
the effective labeled training corpus would multiply several-fold — not in raw tokens,
but in meaningful, independently labeled dimensions of training signal.
The computational cost of this decomposition is significant but tractable. Running a
SigLIP forward pass costs approximately 10ms per frame on modern GPU hardware.
Processing the entire stock of YouTube video (estimated at 800 million hours) at 2
frames per second through a single embedding model would require approximately
4.6 million GPU-hours — substantial, but well within the compute budgets of frontier
AI labs that are spending billions on training runs. And the decomposition only needs
to be done once per data point; the derived embeddings can be stored and reused
across multiple training runs.
7.3 Domain-Specific Applications
The TCT framework is domain-agnostic because it operates entirely in embedding
space. The same principle — decompose through frozen models, compute centroids,
14
constrain to the manifold, validate at runtime — applies wherever independent embedding models exist for the relevant dimensions of meaning:
Language model alignment: Embed model outputs through multiple frozen classifiers (toxicity, factuality, helpfulness, formality). The constellation defines “aligned
text” as a region in the joint space of these classifiers. Every output is validated
against the constellation boundary before release.
Audio/music generation: Embed generated audio through genre classifiers, mood
detectors, audio quality models, and rhythm analyzers. The constellation defines the
target style as a geometric constraint.
Scientific discovery: Embed candidate molecules, materials, or designs through
multiple property predictors. The constellation defines “viable candidate” as proximity to reference examples across all property dimensions simultaneously.
Code generation: Embed generated code through static analysis models, testcoverage predictors, style classifiers, and vulnerability scanners. The constellation
defines “production-quality code” as a measurable region in the joint embedding
space of these quality dimensions.
In every case, the raw data is not changed. The training signal is multiplied by extracting more meaning from the same observations.
8. The Meaning Compression Thesis
9.1 The Compression Landscape in AI
The AI industry is obsessed with compression — and for good reason. Every major
bottleneck in the field is fundamentally a compression problem.
Google’s TurboQuant, announced March 2026 and immediately dubbed “the reallife Pied Piper” by the internet, compresses KV-cache memory by 6× with zero accuracy loss using PolarQuant and Quantized Johnson-Lindenstrauss (QJL). The announcement was significant enough to crater memory chip stocks within hours —
Samsung and SK Hynix each dropped ~6% in a single session. Cloudflare’s CEO
called it “Google’s DeepSeek moment.”
But TurboQuant, like all existing compression algorithms in AI, compresses representations — the numerical artifacts that models produce during inference. It makes
models cheaper to run. It does not make them smarter, more capable, or more aligned.
The same is true of the entire compression taxonomy in modern AI:
Weight compression (quantization, pruning, knowledge distillation) reduces the
storage and compute cost of model parameters. GPTQ compresses weights to 4 bits.
SparseGPT removes 50% of weights through structured sparsity. Knowledge distillation transfers capability from a large teacher to a small student. All of these compress
the model’s knowledge — the patterns it has learned — into fewer bits.
Activation compression (KV-cache quantization, prompt compression, context distillation) reduces the memory cost of inference. TurboQuant is the state of the art
15
here. These compress the model’s working memory — the temporary representations
it uses during a single forward pass.
Data compression (tokenization, deduplication, filtering) reduces the volume of
training data without losing information. BPE tokenization compresses text into subword units. Deduplication removes near-duplicate documents. Quality filtering removes low-signal data. These compress the input — the raw material the model learns
from.
DeepMind’s research demonstrated that LLMs are themselves powerful compressors
— Chinchilla models achieved better compression rates on images and audio than
domain-specific algorithms like PNG and FLAC, because language modeling is mathematically equivalent to optimal compression via the equivalence between prediction
and coding. As their paper states, prediction and compression are two sides of the
same coin, connected by Shannon’s information theory. The April 2025 paper “Understanding LLM Behaviors via Compression” formalized this further, showing that
LLMs learn syntax first (high-frequency patterns compress easily) and factual knowledge second (rare patterns require more capacity).
9.2 What No Existing Algorithm Compresses: Meaning
Every compression algorithm in the current landscape operates on the same axis: reducing the number of bits required to represent a fixed amount of information. TurboQuant compresses KV-cache vectors. GPTQ compresses weights. BPE compresses
text. They all ask the same question: how can we store this information in fewer bits?
Derived Data Abundance asks the opposite question: how can we extract more
meaning from a fixed number of bits?
This is not compression in the traditional sense of reducing size. It is compression
in the information-theoretic sense of increasing the ratio of meaningful signal to raw
data. When ClipCannon runs a 16-minute video through 7 embedding models and produces 12,000+ labeled training samples across 4,044 dimensions, it has not reduced
the size of the video. It has increased the meaning density of the data by extracting
dimensions of semantic content that were present in the raw signal but invisible to
any single analysis.
When Context Graph runs a sentence through 13 embedders and produces 13 independent representations plus 78 cross-correlations, it has not compressed the sentence. It has decompressed its meaning — revealing the full multi-dimensional semantic structure that a single embedding can only partially capture.
This is a fundamentally different operation from anything in the current compression
taxonomy. I propose the term meaning compression to describe it: the process
of transforming raw data into a representation that captures maximal meaning per
unit of information, by projecting through multiple independent measurement models
simultaneously.
16
9.3 The Hierarchy of Compression in AI
With this framing, the compression landscape in AI can be organized into a hierarchy
of increasing value:
Level 1 — Bit Compression. Reduce the storage cost of fixed information. Huffman
coding, gzip, PNG, FLAC. Lossless. Measured in compression ratio (bits out / bits in).
This is classical information theory. Value: saves storage and bandwidth.
Level 2 — Knowledge Compression. Reduce the parameter cost of learned knowledge. Quantization, pruning, distillation. Lossy (but ideally with minimal accuracy
degradation). Measured in performance retention at reduced model size. This is what
TurboQuant and the entire LLM compression field does. Value: saves compute and
memory at inference time.
Level 3 — Meaning Compression. Increase the semantic density of training data by
extracting multiple dimensions of meaning from each data point. Multi-modal embedding decomposition, constellation construction, cross-embedder analysis. Measured
in meaningful training signals per unit of raw data. This is what Derived Data Abundance provides. Value: multiplies the effective training data without generating any
synthetic data.
The hierarchy is ordered by value because each level operates on a progressively more
fundamental bottleneck. Bit compression saves pennies on storage. Knowledge compression saves dollars on inference. Meaning compression potentially saves billions
on training — because it addresses the data wall itself, not just the cost of processing
data that already exists.
9.4 Why Meaning Compression May Be the Most Valuable
The argument that meaning compression is the most valuable form of compression
rests on a simple economic observation: the AI industry’s most expensive problem is
not inference cost (TurboQuant addresses that) or model size (pruning and distillation
address that) — it is the approaching exhaustion of training data.
Frontier model training runs now cost $1–10 billion. The next generation is projected
to cost $10–100 billion. These costs are driven primarily by the need for more data,
more compute, and more parameters to achieve incremental capability improvements.
If the data runs out — as Epoch AI projects between 2026 and 2032 — the entire
scaling paradigm stalls regardless of how efficiently you can compress weights or
KV-caches.
Meaning compression addresses this directly. If each data point in the existing corpus
can be decomposed into N independently meaningful representations, the effective
training corpus is N times larger without any increase in raw data volume, any generation of synthetic data, or any risk of model collapse. The cost of the decomposition
(running embedding models on existing data) is a one-time fixed cost that is trivial
compared to the cost of training a frontier model.
Consider the arithmetic. Context Graph produces 91 meaningful signals per input
(13 embeddings + 78 cross-correlations). If applied to the estimated 300 trillion
17
token stock of public text, the effective training corpus becomes approximately 27
quadrillion meaningful signals — nearly two orders of magnitude larger than the
raw token count. And unlike synthetic data expansion, every one of those signals is
grounded in a real observation, measured by an independently trained frozen model,
carrying no risk of model collapse.
9.5 Compression Duality: TurboQuant and TCT
There is an elegant duality between Google’s TurboQuant and Teleological Constellation Training. Both operate on high-dimensional vectors. Both use the mathematical
structure of embedding spaces to achieve efficiency. Both are lossless in the dimensions that matter.
But they operate in opposite directions:
• TurboQuant takes high-dimensional vectors (KV-cache entries) and compresses them into fewer bits while preserving their information content.
Direction: meaning → fewer bits.
• TCT takes raw data and decompresses it into high-dimensional vectors across
multiple embedding spaces, revealing information content that was latent in the
original. Direction: raw data → more meaning.
TurboQuant makes AI cheaper to run. TCT makes AI cheaper to train. TurboQuant
compresses the output side of the pipeline. TCT compresses (in the informationtheoretic sense of maximizing signal density) the input side. Together, they address
both ends of the AI cost curve.
The Pied Piper comparison that the internet applied to TurboQuant is apt — but it
applies to only half the problem. TurboQuant is Pied Piper for inference. Derived
Data Abundance, if the thesis of this paper holds, is Pied Piper for training data itself
— and training data is the more expensive problem by orders of magnitude.
9. Relationship to Existing Work
9.1 Contrastive Learning and CLIP
The multi-modal embedding approach shares foundational ideas with contrastive
learning methods like CLIP (Radford et al., 2021), which aligns text and image
representations in a shared embedding space. CLIP demonstrated that cross-modal
alignment enables powerful zero-shot capabilities — classifying images using
arbitrary text labels without task-specific training.
TCT extends this principle in two ways. First, it uses more than two modalities simultaneously (seven, in the current implementation), creating a richer constraint surface.
Second, it uses the aligned embeddings not just for retrieval or classification but for
constraining generative model outputs at every level — architecture, loss function,
and runtime validation.
18
9.2 Self-Supervised Learning and V-JEPA
Meta’s V-JEPA (Video Joint Embedding Predictive Architecture) learns video representations by predicting missing or masked parts of a video in an abstract representation
space, without requiring labels. V-JEPA demonstrates that rich, task-transferable representations can be learned from unlabeled video data — a finding consistent with the
Derived Data Abundance thesis that video contains far more information than any single analysis extracts.
TCT differs from V-JEPA in that it uses multiple independent embedding models rather
than a single unified model, and it uses the embeddings as training constraints rather
than as pre-training objectives. The approaches are complementary: V-JEPA-style selfsupervised pre-training could itself be one of the embedding models used in a TCT
constellation.
9.3 Synthetic Data and Data Augmentation
Traditional data augmentation (rotation, cropping, noise injection, back-translation)
and modern synthetic data generation (GAN-based, diffusion-based, LLM-based) both
aim to increase the effective training corpus. The key difference from Derived Data
Abundance is the source of the additional data:
• Data augmentation creates variations of existing data points (a rotated version
of an image, a paraphrased version of a sentence). The variations are grounded
in the original data but carry limited new information.
• Synthetic generation creates entirely new data points from a model’s learned
distribution. The new data carries the model’s compression artifacts and is susceptible to model collapse.
• Derived data extracts new dimensions of meaning from existing data points using independent measurement models. Each dimension is a new training signal
grounded in the original observation.
The distinction matters because it determines whether the additional data carries new
information or amplified noise. Derived data carries new information by construction:
each embedding model was trained to extract a different dimension of meaning, so
its output is informationally independent of the other models’ outputs (to the extent
that the underlying training distributions were independent).
10. The Unified Theory of Learning Connection
The Derived Data Abundance principle connects to a broader theoretical framework:
the Unified Theory of Learning (UTL), which formalizes learning as the product of
surprise (Delta-S, how unexpected an experience is) and coherence (Delta-C, how
well the experience integrates into existing knowledge).
In the UTL framework, optimal learning occurs when both surprise and coherence are
moderate — the learner encounters something unexpected but can make sense of it
19
within a coherent framework. The multiplicative relationship (L = Delta-S × DeltaC) means that either signal alone is insufficient: pure surprise without coherence is
confusion; pure coherence without surprise is boredom.
Multi-modal embedding decomposition serves the UTL equation directly. By providing training data labeled across multiple independent dimensions, the decomposition
increases both the surprise signal (each modality reveals a different dimension of the
data, providing unexpected information to a model trained on fewer modalities) and
the coherence signal (the multiple labels for each data point provide a rich, internally consistent framework for integrating the information). The product of higher
surprise and higher coherence produces more efficient learning — which is precisely
what TCT demonstrates empirically.
The implication is that the data crisis is not just a volume problem but a learning efficiency problem. Models trained on one-dimensional data (text tokens, pixel values)
are operating in the low-coherence regime of the UTL curve: each data point carries
a single dimension of meaning, so the model must see many data points to extract
the multi-dimensional structure of the world. Models trained on multi-dimensionally
labeled data operate in the high-coherence regime: each data point carries rich, structured information that the model can integrate efficiently.
This suggests that the scaling laws themselves may shift when training data is multidimensionally enriched. The “tokens needed for a given performance level” metric assumes one-dimensional training data. With N-dimensional embedding-enriched data,
each token carries N times the meaningful signal, potentially shifting the scaling
curve by the same factor.
11. Implications for the AI Industry
12.1 The Data Wall Is a Labeling Wall
Reframing the data crisis as a labeling crisis rather than a volume crisis changes the
solution space entirely. The internet contains hundreds of exabytes of data. The problem is not that this data doesn’t exist — it is that most of it is unlabeled, unstructured,
and one-dimensional from a training perspective. Multi-modal embedding decomposition addresses all three limitations simultaneously: it labels (each embedding is a
label), structures (the embeddings define a geometric space), and adds dimensions
(each model adds an independent dimension of meaning).
12.2 Embedding Models as Data Multipliers
The implication for AI research priorities is significant. Instead of investing primarily in larger generative models or more compute for pre-training, the industry should
also invest in better and more diverse embedding models — because each new embedding model that captures a genuinely independent dimension of meaning effectively
multiplies the available training data by one factor.
A research program focused on developing 50 or 100 independent, high-quality embedding models across different perceptual and cognitive dimensions — visual, au20
ditory, linguistic, emotional, temporal, spatial, causal, intentional, social — would
create the infrastructure for a massive expansion of effective training data without
any increase in raw data volume.
12.3 The Infrastructure for Derived Data
Realizing the vision of Derived Data Abundance at scale requires infrastructure that
does not yet exist:
Embedding pipelines that can process internet-scale data through multiple models
efficiently. ClipCannon demonstrates the pipeline architecture at single-video scale;
scaling to billions of data points requires distributed processing and efficient storage
of high-dimensional vectors.
Vector databases capable of storing and querying trillions of embedding vectors
across dozens of modalities. The sqlite-vec extension used by ClipCannon works at
project scale; internet scale requires purpose-built distributed vector storage.
Constellation construction tools that compute and manage multi-modal centroids
from large reference datasets. The current implementation handles single-subject
constellations; scaling to millions of constellations (one per concept, entity, or behavioral pattern) requires new algorithms for centroid management and hierarchical
constellation structures.
Runtime validation systems that can check every model output against its constellation in real time. The current constellation guard runs at approximately 10ms per
frame on an RTX 5090; web-scale validation requires parallelized validation infrastructure.
12. Limitations and Open Questions
12.1 Embedding Model Quality
The Derived Data Abundance principle assumes that embedding models capture genuinely independent dimensions of meaning. In practice, embedding models trained on
overlapping data may produce correlated outputs, reducing the effective information
gain. Research is needed to quantify the degree of independence between embedding models and to develop models specifically designed for orthogonal information
extraction.
12.2 Computational Cost
Processing existing data through multiple embedding models is computationally expensive. While tractable for single projects or moderate-scale datasets, internet-scale
decomposition would require substantial compute investment. The economics depend on whether the derived data produces sufficient training improvements to justify
the processing cost.
21
12.3 Threshold Calibration
The constellation guard requires per-modality thresholds that must be tuned empirically. There is currently no principled method for selecting optimal thresholds from
first principles. Research into automatic threshold calibration — perhaps using the
spread of the reference embeddings as a guide — would strengthen the framework.
12.4 Scalability of Centroid Computation
The current centroid computation (L2-normalized mean of L2-normalized samples) is
simple and effective for small datasets. At scale, with millions of reference points per
modality, more sophisticated centroid estimation methods (perhaps robust statistics
or mixture models) may be needed to handle multimodal distributions within a single
modality.
12.5 The Limits of Embedding Completeness
The argument that “anything you can embed, you can constrain” assumes that embedding models provide approximately complete descriptions of their input domains.
This assumption holds reasonably well for mature embedding models (SigLIP for images, WavLM for audio) but may not hold for less-studied modalities (touch, smell,
social context). The framework’s power is limited by the quality of available embedding models.
13. Conclusion
The AI industry’s training data crisis is real but misunderstood. The problem is not
that we are running out of data — the internet contains more data than any model
can consume. The problem is that we are running out of one-dimensional data: text
treated as token sequences, images treated as pixel arrays, each carrying a single
dimension of training signal.
The solution is not to generate more data synthetically (which risks model collapse)
or to find new caches of raw human content (which is finite). The solution is to extract more meaning from the data we already have, using the proliferating ecosystem
of frozen embedding models as measurement instruments that reveal dimensions of
meaning invisible to any single analysis.
ClipCannon demonstrates this principle on video: from 16 minutes of footage,
12,000+ labeled training samples across 4,044 dimensions. Context Graph demonstrates it on text: every memory analyzed through 13 independent embedders
producing 13 representations plus 78 cross-correlations — a 91× multiplication in
meaningful signal from a single input. Teleological Constellation Training demonstrates the application: a generative model aligned to a specific identity through
geometric constraint in multi-modal embedding space, with per-output verification
guarantees that no scalar reward system can match.
22
The broader principle — Derived Data Abundance — states that the effective training data available from a fixed corpus grows multiplicatively with the number of independent embedding models applied to it. Each model extracts a different dimension
of meaning. Each dimension is a new training signal. Each signal is grounded in
real observation, not model generation. Two independent systems — one processing
video through 7 modalities in Python, one processing text through 13 embedders in
Rust — demonstrate that this principle is universal: it works on any data type, for
any number of embedding models, in any combination. It is not a multimodal technique. It is a data enrichment technique that happens to work spectacularly well
across modalities, within a single modality, and across data types of all kinds.
The data wall is not a wall. It is a door — and the key is multi-modal embedding
decomposition. The data we have, properly decomposed, is more than enough to train
the next generation of AI systems. We just need to learn to see all the dimensions of
meaning that each data point contains.
References
• Hoffmann, J., Borgeaud, S., Mensch, A., Buchatskaya, E., Cai, T., Rutherford, E.,
de Las Casas, D., Hendricks, L. A., Welbl, J., Clark, A., Hennigan, T., Noland,
E., Millican, K., van den Driessche, G., Damoc, B., Guy, A., Osindero, S., Simonyan, K., Elsen, E., Rae, J. W., Vinyals, O., & Sifre, L. (2022). “Training
Compute-Optimal Large Language Models.” arXiv:2203.15556. Available at:
https://arxiv.org/abs/2203.15556
• Kaplan, J., McCandlish, S., Henighan, T., Brown, T. B., Chess, B., Child, R., Gray,
S., Radford, A., Wu, J., & Amodei, D. (2020). “Scaling Laws for Neural Language
Models.” arXiv:2001.08361. Available at: https://arxiv.org/abs/2001.08361
• Shumailov, I., Shumaylov, Z., Zhao, Y., Papernot, N., Anderson, R., & Gal, Y.
(2024). “AI models collapse when trained on recursively generated data.” Nature, 631(8022), 755–759. https://doi.org/10.1038/s41586-024-07566-y
• Villalobos, P., Ho, A., Cevallos, J., Suárez, A., & Heim, L. (2024). “Will we
run out of data? Limits of LLM scaling based on human-generated data.”
arXiv:2211.04325. Available at: https://arxiv.org/abs/2211.04325
• Radford, A., Kim, J. W., Hallacy, C., Ramesh, A., Goh, G., Agarwal, S., Sastry, G., Askell, A., Mishkin, P., Clark, J., Krueger, G., & Sutskever, I. (2021).
“Learning Transferable Visual Models From Natural Language Supervision.”
arXiv:2103.00020. Available at: https://arxiv.org/abs/2103.00020
• Ekman, P., & Friesen, W. V. (1978). Facial Action Coding System: A Technique
for the Measurement of Facial Movement. Consulting Psychologists Press, Palo
Alto, CA.
• Li, T., Bolkart, T., Black, M. J., Li, H., & Romero, J. (2017). “Learning a Model
of Facial Shape and Expression from 4D Scans.” ACM Transactions on Graphics
(TOG), 36(6), Article 194. https://doi.org/10.1145/3130800.3130813
• Hu, E. J., Shen, Y., Wallis, P., Allen-Zhu, Z., Li, Y., Wang, S., Wang, L., &
Chen, W. (2021). “LoRA: Low-Rank Adaptation of Large Language Models.”
arXiv:2106.09685. Available at: https://arxiv.org/abs/2106.09685
23
• Zhai, X., Mustafa, B., Kolesnikov, A., & Beyer, L. (2023). “Sigmoid Loss for Language Image Pre-Training.” arXiv:2303.15343. Available at: https://arxiv.org/abs/2303.15343• Bardes, A., Garrido, Q., Ponce, J., Chen, X., Rabbat, M., LeCun, Y., Assran, M., &
Ballas, N. (2024). “Revisiting Feature Prediction for Learning Visual Representations from Video.” Meta AI Research. Available at: https://ai.meta.com/blog/vjepa-yann-lecun-ai-model-video-joint-embedding-predictive-architecture/
• Royse, C. (2026). “ClipCannon and the Teleological Constellation System: A Unified White Paper on Identity-Aligned Video Understanding, Editing, and Avatar
Generation.” White Paper, April 2026.
• Royse, C. (2026). “Teleological Constellation Training: Identity-Aligned Video
Generation Through Multi-Modal Embedding Anchors.” Technical Paper, April
2026.
• Royse, C. (2026). “The Unified Theory of Learning (UTL): A First-Principles
Framework for How Learning Actually Works.”
• Royse, C. (2026). “Context Graph: A 13-Embedder Semantic Memory Retrieval
System.” System Documentation, April 2026.
• Google Research. (2026). “TurboQuant: Redefining AI Efficiency with Extreme Compression.” Google Research Blog, March 24, 2026. Available at:
https://research.google/blog/turboquant-redefining-ai-efficiency-with-extremecompression/
• Deletang, G., Ruoss, A., Grau-Moya, J., Genewein, T., Wenliang, L. K., Catt, E.,
Cundy, C., Hutter, M., Legg, S., Veness, J., & Ortega, P. A. (2024). “Language
Modeling Is Compression.” In Proceedings of ICLR 2024. arXiv:2309.10668.
Available at: https://arxiv.org/abs/2309.10668
• Li, C., et al. (2025). “Lossless data compression by large models.” Nature Machine Intelligence. https://doi.org/10.1038/s42256-025-01033-7
End of paper.
24