---
namespace: research
agent_id: agent-00-orchestrator
timestamp: 2026-04-19T04:00:00Z
status: active
tags: addendum, operational-capabilities, clipcannon, phoenix, voiceagent, production-substrate
---

# Capabilities Addendum — What Actually Ships Today (exploratory proofs of the primitive)

**Source**: `tasks/newdata/alldata.md` (Chris's April 2026 unified white paper on
ClipCannon + Phoenix + VoiceAgent + Teleological Constellation System).
**Purpose**: this addendum captures the **operational substrate behind the three exploratory
proofs of the primitive**. Every case study documented here was built to prove the primitive
(DDA / meaning compression / TCT — see `THE_PRIMITIVE.md`) produces deployable results across
independent modalities. **These systems are not products being pitched.** They are evidence that
the primitive is real. Use this whenever a target asks "OK but what actually runs?" — the
answer is "these exploration probes do; the real asset is the primitive that made them
possible."

---

## 1. The Operational Stack (three packages, one wheel)

### ClipCannon — video understanding + editing + rendering + audio + voice + avatar
- **Scale**: 247 source files, **67,585 lines of Python**, **58 MCP tools**, **23-stage analysis DAG**, 34 core DB tables + 4 sqlite-vec vector tables, **1,131 tests** across 85 test files
- **Entry**: MCP server (stdio or SSE:3366), license server FastAPI (port 3100), dashboard (port 3200)
- **Per-project DB**: SQLite WAL, schema v4, with `vec_frames (float[1152])`, `vec_semantic (float[768])`, `vec_emotion (float[1024])`, `vec_speakers (float[512])`
- **Three-tier storage**: sacred (analysis.db + source) / regenerable (CFR + stems + frames + storyboards) / ephemeral
- **License**: BSL-1.1 · Python ≥ 3.12 · Hatchling build

### Phoenix — GPU-native avatar engine
- **FLAME fitter** at **82 fps on RTX 5090** (5,023 vertices, 100 expression coefficients, 3D jaw/neck/eye pose)
- **PhysicsFaceEngine**: audio formants (F1/F2/F3) → face params deterministically in **<5 ms/chunk**, language-agnostic, 34 unit tests
- **ConstellationController**: 4-level expression hierarchy (17 Action Units → 40 K-Means Groups → 34 Skills → **8 named Constellations**)
- **ConstellationGuard**: runtime per-frame SigLIP re-embedding + cosine-to-centroid check per modality (visual 0.70 / emotion 0.60 / speaker 0.80 / voice 0.85); WARN/STRONG visual tiers at 0.80/0.90
- **gpu_guard** for WSL2 stability: threading lock serialises all GPU ops, aggressive `gc.collect + torch.cuda.empty_cache` cleanup, VRAM-budget enforcement, forced offline mode (`HF_HUB_OFFLINE=1`), pre-`torch`-import CUDA config

### VoiceAgent — real-time meeting bot
- **Live on Google Meet + Zoom** as an AI-clone participant
- **Reasoning controller** (Gemma 4) selects active constellation per scene
- Standalone deployment (separate from ClipCannon wheel)

---

## 2. The 7-Modality Constellation (4,044 total dimensions)

| # | Modality | Model | Dim | Captures |
|---|---|---|---|---|
| 1 | Visual | SigLIP-SO400M-patch14-384 | 1,152 | Face, pose, expression |
| 2 | Semantic | Nomic-embed-text-v1.5 | 768 | Spoken content, topics |
| 3 | Emotion | Wav2Vec2-large-emotion | 1,024 | Vocal emotion, arousal, valence |
| 4 | Speaker | WavLM-large | 512 | Voice identity, timbre, formants |
| 5 | Prosody | Custom (F0 / energy / rate / contour) | 12 | Speaking style |
| 6 | Sentiment | all-MiniLM-L6-v2 | 384 | Sentence-level sentiment, intent |
| 7 | Voice | ECAPA-TDNN | 192 | Speaker verification |

**Constellation stability**: frozen centroids. **Decomposability**: each modality is an independent
constraint — drift in one is detectable even if others are satisfied. **Density**: 4,044D is
sparse enough to allow within-identity variation and dense enough to catch out-of-distribution
drift.

**Sample counts from the Santa 975-second (16m15s) interview after diarisation**:
visual 1,725 · emotion 343 · speaker 300 · sentiment 202 · voice 188 · prosody 188 · semantic 200.
L2-normalise then mean-then-normalise produces each centroid.

---

## 3. The Clone Video Pipeline (Case 2, operational)

```
Source video (16 minutes of subject)
    │
    ▼  ClipCannon 23-stage DAG (probe → VFR normalise → audio extract → frame extract
    │   → transcribe (WhisperX large-v3, 35-phrase anti-hallucination filter) → source
    │   separation (Demucs) → semantic embed → emotion embed → speaker embed → prosody
    │   → scene analysis → narrative LLM (Qwen3-8B) → highlights → finalise)
    │
    ▼  Embedding extraction → all_embeddings.npz (7 modalities, numbers above)
    │
    ▼  Constellation construction → frozen centroids
    │
    ▼  Curated training data: 2,362 clips, 1.96s @ 25 fps, 13-frame temporal latent,
    │   mouth mask from YOLOv5-Face 5-point landmarks, 15-viseme sequence from CMU dict,
    │   7,916 real samples, 100% viseme coverage
    │
    ▼  LoRA training: EchoMimicV3 (1.86B DiT, Wan2.1-Fun base), rank 256 α 512, 8 target
    │   modules (q k v o k_audio v_audio k_img v_img — q_audio excluded due to upstream
    │   typo bug), 3,000 steps, LR 1e-4 cosine, mouth-weighted flow-matching (10× mouth /
    │   2× face / 1× background = ~19% effective mouth gradient), WeightedRandomSampler
    │   on viseme rarity (OY/AW/UH 2-4× oversampled), ~90-120 min on RTX 5090, <14 GB VRAM
    │
    ▼  Inference: ConstellationController → per-frame NL prompts; Qwen3-TTS cloned voice;
    │   reference image (768×768 face-centred); Flow_DPM++ 20 steps; multi-chunk 10-s
    │   generation with 0.5-s audio overlap + pixel crossfade
    │
    ▼  ConstellationGuard.validate_batch → per-frame cosine ≥ 0.70 visual, per-modality
        thresholds above; regenerate on fail, fall back on budget exhaustion, escalate
        to operator on sustained drift
```

**Six forensic fixes applied and individually verified**:
1. Flow-matching sign bug (`target = noise - latent`, not `latent - noise` — EchoMimicV3 velocity convention)
2. `cv2` 60-fps read → explicit `ffmpeg -r 25` (clips were 2.4× too short relative to audio)
3. Mouth bbox from YOLOv5-Face 5-point landmarks (earlier geometric-ratio bboxes landed on nose)
4. `q_audio` LoRA target excluded (upstream `q_auido` typo at line 843 overwrote the output)
5. Audio features streamed to disk per-clip (earlier 22 GB RAM accumulation)
6. `WeightedRandomSampler` with viseme-rarity weights (earlier uniform despite "weighted" advertised)

---

## 4. The Voice Cloning Pipeline (Case 3, operational)

- **Qwen3-TTS** base inference, Qwen3-TTS **ECAPA-TDNN 2048-dim** speaker embeddings in `~/.clipcannon/voice_profiles.db`
- **Multi-gate verification**: sanity (duration / clipping / SNR / leading silence) → intelligibility (WhisperX WER w/ punctuation stripping) → identity (SECS against profile centroid)
- **Hard floor**: **SECS ≥ 0.95 at all times** — non-negotiable behavioral rule
- **Optimised synthesis**: generate N candidates → select highest SECS → Resemble Enhance denoise + latent flow-matching upsample (24 kHz → 44.1 kHz broadcast)
- **Prosody-aware reference selection**: auto-pick reference clips matching delivery style (energetic / calm / emphatic / varied / fast / slow / rising / question / best) from the `prosody_segments` table
- **Multi-voice synthesis**: swap profiles mid-conversation for dialogue

---

## 5. The Provenance Chain (enterprise-ready)

Every pipeline operation produces a row in the `provenance` table with 22 columns incl.
`record_id`, `parent_record_id`, `input_sha256`, `output_sha256`, `model_name`, `model_version`,
`model_quantization`, `model_params` (JSON), `chain_hash`. The chain hash is:

```
SHA256("{parent_hash}|{input_sha256}|{output_sha256}|{operation}|{model_name}|{model_version}|{json(params,sort_keys=True)}")
```

Genesis is a hard-coded sentinel. `verify_chain` walks the chain chronologically, recomputes
each hash, and identifies any broken record. **Detects accidental corruption and unauthorised
partial edits.** Does not detect a privileged actor who recomputes the whole chain from
genesis with modified data — that would require digital signatures, intentionally out of scope
(threat model is integrity verification, not adversarial tamper resistance).

**Why this matters for sovereign / enterprise / regulated-vertical outreach**: the provenance
table is a **compliance artefact** ready for audit. Every generated clip, every LoRA step,
every voice synthesis links back to source through a verifiable hash chain. This is not a
research prototype — it is an auditable production system.

---

## 6. The HMAC Billing System (shipping commercial infra)

- **License server** on port 3100 (FastAPI + Uvicorn + SQLite at `~/.clipcannon/license.db`)
- **Balance integrity**: `HMAC-SHA256(machine_key, "balance:{balance}")` with deterministic
  per-machine key `SHA256("{hostname}|{mac}|{cpu_arch}")[:32]`; versioned salt
  `"clipcannon-v1|{machine_id}"` enables key rotation; constant-time comparison via `hmac.compare_digest`
- **Tamper detection is fatal** — mismatch returns HTTP 403 `BALANCE_TAMPERED`, no recovery
- **Credit rates** (analyze 10 / render 2 / metadata 1 / publish 1), packages 50/$5 → 5,000/$200
- **Stripe webhook**: `checkout.session.completed` processed in `stripe_webhooks.py`
- **Spending limits**: warn at 80% cap, block at cap with `SPENDING_LIMIT_EXCEEDED`

---

## 7. The Verification Surface (750 FSV checks)

Four primary Full State Verification scripts run **750 individual checks** across the stack:
- Core infrastructure (154)
- Pipeline + tools (189)
- Billing + dashboard (84)
- Server integration (323)

Six extended FSV scripts add additional coverage. **Every FSV script assumes code is guilty
until proven innocent** and requires evidence (DB state, file existence, hash verification)
for every PASS — documented as the project's forensic-investigation pattern. This is the
empirical substrate behind the paper's §9 EXP-1/2/3 falsifiability claim.

---

## 8. Seventeen Failed Approaches (documented negative results)

Over a six-day sprint Chris attempted 17 architectures before landing on the operational stack.
Negative-result examples:
- CogVideoX-2b / 5b-I2V LoRAs (8 fps, eye artefacts)
- Gaussian splatting (blurry blobs even at 50,000 iterations; WSL2-unstable)
- MuseTalk 1.5 / LatentSync (bearded-subject face-detection failure; per-frame affine jitter)
- EchoMimicV3 LoRA v1–v5 (generic lip-sync defeated by flow-matching sign bug — later fixed)
- Hallo3 14.55B + HunyuanVideo-Avatar 14.55B (VRAM / latency budget insufficient)
- Perceiver clone model with North Star embeddings (deleted after Gaussian failure)
- Face warper / NVENC encoder bespoke renderer (removed, LatentSync absorbed function)

These are the **engineering-rigour anchor** for outreach. When a reviewer asks "why should this
particular approach work" the answer is "because seventeen didn't" — with forensic documentation
per approach.

---

## 9. What to Say in Outreach

When asked "what actually runs today?", a correct answer is:

> "ClipCannon + Phoenix + VoiceAgent. 67,585 lines of Python, 58 MCP tools, a 23-stage
> analysis DAG, a 7-modality 4,044-dimensional identity constellation with runtime cosine
> guarding per frame, a tamper-evident SHA-256 provenance chain on every operation, 1,131
> passing tests, and 750 full-state-verification checks. Case 1 Shakespeare LoRA is public on
> HuggingFace. Case 2 Santa clone-video ships end-to-end through the stack — the paper draft
> §6.2 tag is paper-internal only. Case 3 voice clone hits 0.961 mean WavLM SECS within-
> WavLM-family on 10 held-out sentences. Live demo on any of the three on 24-hour notice."

Every claim above is scope-preserving (paper §8 limits all stand) and every number is MEASURED
on the operational stack.

---

## 10. What to Offer

- **30-minute live demo of Case 2 pipeline**: ingest a non-sensitive reference clip → show the
  constellation construction → run a generation pass → show runtime guard rejecting out-of-
  distribution frames.
- **30-minute Phoenix avatar + VoiceAgent demo**: real-time meeting-bot attendance of a Google
  Meet call as a configurable identity.
- **48-hour on-prem PoC**: target delivers a corpus + embedder panel; Chris delivers a
  DDA-projected signal count + trained `G_τ` guard + per-output cosine trace. Target data
  never leaves their environment.
- **Co-authorship on EXP-1 (ECAPA-TDNN cross-encoder re-score), EXP-2 (pairwise-MI audit on
  the 13-embedder panel), or EXP-3 (nm sweep)**.

---

## 11. Scope Guards (unchanged — paper §8)

All scope guards stated elsewhere in the package apply here without modification:
- Case 3 = within-WavLM-family, N=10, one speaker, English (ECAPA-TDNN EXP-1 pending)
- "Up to 91 signals / input" = constructive upper bound, EXP-2 pending
- DDA structurally outside Shumailov regime = scope, not refutation
- TCT vs scalar-reward = different property class, not a benchmark contest
- Case 2 paper-draft tag: paper-internal only; operational substrate is MEASURED 2026-04-19

---

## 12. Competing Interests

Chris Royse is founder/operator of Teleox.ai (primary) and Leapable.ai (companion MCP
marketplace). This addendum emerges from commercial engineering practice. Asymmetric release
posture remains: Case 1 byte-identical release; Case 2 and Case 3 subject artefacts withheld
under dual-use.
