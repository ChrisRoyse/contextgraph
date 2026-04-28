---
namespace: reports
agent_id: agent-27-instrument-developer
timestamp: 2026-04-16T16:40:00Z
status: active
tags: final-report, smap, instruments, section-5, section-6, section-8
depends_on:
  - memory/decisions/agent-27-instrument-developer--instrument-specifications.md
  - memory/patterns/agent-27-instrument-developer--instrument-to-case-map.md
  - memory/discoveries/agent-27-instrument-developer--instrument-limitations.md
  - memory/handoff/agent-27-to-28--instruments-ready.md
downstream_consumers:
  - agent-28-analysis-planner
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
  - agent-43-reproducibility-checker
---

# Agent-27 Final Report — SMAP

## S — Situation

Upstream agents (agent-25 method-designer, agent-26 sampling-strategist)
have locked:
- The TCT three-phase protocol and Case-1 / 2 / 3 methodology.
- Populations, sample sizes, disjointness matrix, held-out sets,
  exploratory-N = 1 framing, scope-of-validity paragraphs.

What was missing for §5 (Methods) and §6 (Case studies) to be
writeable: **the byte-level specification of each measurement
instrument** — exact model repo, preprocessing, forward-pass output
format, L2-norm / cosine / pooling conventions, operating thresholds,
and the limitation disclosures §8 must carry.

## M — Mission

Specify the measurement instruments used in each case study to the
point where a replicator can compute every §6 number byte-identically
from the same inputs, and produce the §8 paste-ready limitations
register that accompanies each instrument.

Concretely:
- **Identity instruments** (A): WavLM SECS (Case 3 guard + evaluator;
  Case 2 speaker modality), ECAPA-TDNN SECS (Case 3 W1 remediation),
  Qwen3-TTS internal speaker encoder (documented-not-guard).
- **Naturalness instruments** (B): DNSMOS P.808 / P.835 (Case 3),
  UTMOS (Case 3).
- **Visual instruments** (C): SigLIP-SO400M visual centroid cosine
  (Case 2 global visual), FLAME 100-D expression coefficients
  (Case 2 action-unit vocabulary, NOT a guard).
- **Text-style instruments** (D): STYLE_CENTROID 9-embedder
  constellation (Case 1), vocabulary-density prefilter (Case 1).
- **Lip-sync instruments** (E): LSE-D / LSE-C (Case 2, PENDING).
- **Provenance instruments** (F): SHA-256 content hashing.

## A — Action

### A.1 Instrument specifications (primary deliverable)

File: `decisions/agent-27-instrument-developer--instrument-specifications.md`

Locked 15 instruments across six categories with:
- Exact HuggingFace / repo IDs (with revision / commit hashing via
  SHA-256 over weights at run time).
- Canonical preprocessing pipelines (sample rates, channels,
  normalisation, silence trim, crop, tokeniser).
- Forward-pass specifications (which head / output to read).
- Output format (dimension, dtype, range).
- L2-norm and cosine conventions (per-sample L2, then per-sample
  mean, then L2 the mean → centroid; cosine formula explicit).
- Pooling (per-frame for SigLIP → per-clip mean → centroid double-L2).
- Operating thresholds and their calibration procedure (TCT §3.3
  10th-percentile same-session-band floor, with V12 hedge on
  author-observed band).
- Units and direction of merit.
- Per-instrument limitation codes.

**Key invariants locked**:
- WavLM-SECS: `microsoft/wavlm-base-plus-sv`, 16 kHz mono, peak-to-
  0.95, silence-trim −40 dBFS, 512-D output, raw cosine in [−1, +1].
- ECAPA-TDNN SECS: `speechbrain/spkrec-ecapa-voxceleb`, same audio
  preprocessing, 192-D output.
- DNSMOS: `microsoft/DNS-Challenge` ONNX, 16 kHz mono, 9 s frames
  with 1 s overlap, NO loudness normalisation (that would alter
  scores), mean over frames.
- UTMOS: `sarulab-speech/UTMOS22` strong variant, 16 kHz mono, no
  loudness normalisation.
- SigLIP: `google/siglip-so400m-patch14-384`, RGB 384×384 centre
  crop, canonical mean/std [0.5, 0.5, 0.5], pooler_output 1,152-D.
- STYLE_CENTROID: 9 of 13 Context Graph embedders (E1, E4, E6, E7,
  E8, E9, E10, E12, E13); sparse and late-interaction scoring
  conventions preserved per embedder; per-embedder CV reporting.
- SHA-256: `raw_file_sha256` AND `pcm16k_float32_le_sha256` paired
  (so audit survives resampling / decoding).

### A.2 Instrument-to-case map (writer reference)

File: `patterns/agent-27-instrument-developer--instrument-to-case-map.md`

Forward map (per case) and reverse map (per instrument). Canonical
phrasing register ("WavLM SECS", "DNSMOS P.808", etc. — never bare
"SECS" or "DNSMOS"). Per-section writer instructions for §5 / §6.1
/ §6.2 / §6.3 / §8.

### A.3 Limitation register (§8 paste-ready)

File: `discoveries/agent-27-instrument-developer--instrument-limitations.md`

32 limitation codes across 11 instrument families. Each entry is a
verbatim-paste sentence for agent-40. Top-priority ordering:
1. **L-SECS-A** — WavLM is Case-3 guard AND evaluator → self-
   consistency, not cross-encoder.
2. L-ECAPA-A — ECAPA on VoxCeleb, systematic offset from WavLM.
3. L-DNSMOS-A / L-UTMOS-C — parity not absolute.
4. L-STYLE-A — Case 1 cross-encoder pending.
5. L-LSE-C — Case 2 lip-sync deferred.
6. L-Qwen3-B — reject Qwen3-as-cross-encoder framing.
7. L-SHA-A — byte-exact not semantic.

### A.4 Reproducibility JSON manifest schema

Embedded in §H of the specifications file. Every Case 3 headline
number is reconstructable from the manifest:
- `embedders.<name>.weight_sha256`
- `reference_set_manifest.items[*].pcm16k_float32_le_sha256`
- `centroid_hash`
- `threshold_set_hash`
- `per_output_trace[*].cosines.<modality>`
- `derived_metrics.wavlm_secs_mean` (etc.)

Agent-43 (reproducibility-checker) uses this schema.

### A.5 Screening-gate operationalisations

Per agent-26 §E, operationalised in §G of the specifications:
- Case 1: Folger / Gutenberg validator, FSV F5 duplicate-scan.
- Case 2: YOLOv5-Face + WavLM-large diarisation + `prosody_segments`
  + scene-disjointness.
- Case 3: SNR estimator, length gate, single-speaker check,
  lemma-overlap checker (spaCy).

### A.6 Handoff to agent-28 (analysis-planner)

File: `handoff/agent-27-to-28--instruments-ready.md`

Specifies what agent-28 must design next: statistical machinery
(bootstrap CI, cross-encoder agreement, per-embedder CV,
histogram bin choice, conjunction accept-rate CIs, multiplicity
correction), §6 tables / figures plan, reproducibility-check
plan, writer contracts for agent-37 / agent-38 / agent-40.

Open carries flagged: Case-2 emotion encoder model-ID, Case-2
voice-identity encoder model-ID, post-submission run-plans for
W1 / W2 / W3 remediation, human-Likert in-scope decision.

## P — Problems / PENDING items

- **Case-2 emotion encoder**: not yet nailed to a specific
  HuggingFace model ID by upstream; agent-28 or agent-37 must
  resolve. Candidates: audio emotion encoders
  (`ehcalabres/wav2vec2-lg-xlsr-en-speech-emotion-recognition`)
  or multi-modal emotion encoders. Either pick-and-document or
  mark "encoder TBD" in §6.2.
- **Case-2 voice-identity encoder**: similarly open. Default to
  WavLM (shared with speaker) and collapse to one modality, OR
  add a distinct encoder (Resemblyzer, pyannote/embedding) and
  document the architectural independence.
- **W1 / W2 / W3 remediation**: Case-3 remediation instruments are
  ready; the runs themselves are not scheduled to complete before
  arXiv v1 submission. Agent-28 must decide in-scope vs. v2.
- **Case-1 and Case-2 T1 end-to-end numbers**: instruments are
  ready; training / evaluation runs are in progress. §6.1 and §6.2
  must carry T1 PENDING flags.
- **Human-Likert protocol** (Case 1 / Case 2): agent-27 recommends
  out-of-scope for arXiv v1; agent-28 to confirm.

## Files written

1. `/home/cabdru/contextgraph/docs/arxivpaper/memory/decisions/agent-27-instrument-developer--instrument-specifications.md`
2. `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-27-instrument-developer--instrument-to-case-map.md`
3. `/home/cabdru/contextgraph/docs/arxivpaper/memory/discoveries/agent-27-instrument-developer--instrument-limitations.md`
4. `/home/cabdru/contextgraph/docs/arxivpaper/memory/handoff/agent-27-to-28--instruments-ready.md`
5. `/home/cabdru/contextgraph/docs/arxivpaper/memory/reports/agent-27-instrument-developer--final-report.md` (this file)

## Key decisions / locks

1. **WavLM-base-plus-sv is the Case 3 primary scorer.** Raw cosine,
   not rescaled. 512-D output. L-SECS-A disclosure is mandatory.
2. **ECAPA-TDNN is W1 remediation, not primary.** Absolute-value
   comparison with WavLM is NOT the audit signal; ranking agreement
   and ΔSECS sign are.
3. **Qwen3-TTS internal 2048-D encoder is documented only.** It is
   NOT a cross-encoder check; framing it as such is a category
   error (L-Qwen3-B).
4. **DNSMOS and UTMOS are parity metrics, not absolute-quality
   scores.** Headline is "clone 3.93 = real 3.93 (P.808)" and
   "clone 3.012 = real 2.997 (UTMOS)" — parity, not rating.
5. **SigLIP-SO400M-patch14-384 is Case-2 global visual; FLAME is
   action-unit vocabulary, not a guard.** Mixing these two would be
   a category error.
6. **STYLE_CENTROID is 9-embedder, not 13.** E2 / E3 / E5 / E11 are
   deliberately excluded (non-stylistic signal).
7. **Content SHA-256 is the integrity backbone.** Paired hashes
   (raw file AND decoded PCM) so audit survives preprocessing.
8. **Operating thresholds are TCT-§3.3-calibrated, not tuned on
   evaluation.** 10th-percentile same-session-band floor.

## Confidence

**HIGH** that the 15 instrument specifications are byte-level
re-implementable from this report's files alone, given public model
repositories and the published citations.

**HIGH** that the L-SECS-A disclosure is the single most important
§8 instrument-level sentence and has been identified as top-priority
for agent-40 to paste.

**HIGH** that the JSON manifest schema is sufficient for agent-43
(reproducibility-checker) to verify the Case-3 headline numbers
(0.961 / 0.975 / 3.93 / 3.012) from a re-run.

**MEDIUM** that Case 2 is fully specified: two modalities (emotion,
voice-identity) have encoder model-IDs as open carries. Agent-28 or
agent-37 resolution is recommended.

## Hedges carried forward (unchanged)

V4 (encoder-naming), V12 (same-session-band framing), V22
(projected tags), V18 (frozen-encoder discipline), Bias-05 / Bias-06
/ Bias-08 / Bias-09, FM-04 / FM-05, MP-12 (composition-not-
invention), R3 / R4 (coverage).

## Changelog

- 2026-04-16 — Initial issue by agent-27-instrument-developer.
