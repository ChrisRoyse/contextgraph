---
namespace: discoveries
agent_id: agent-26-sampling-strategist
timestamp: 2026-04-16T09:15:00Z
status: active
locked: true
supersedes: none
depends_on:
  - memory/decisions/agent-26-sampling-strategist--sampling-protocols.md
  - memory/decisions/agent-25-method-designer--case-study-protocols.md
  - memory/discoveries/agent-14-bias-detector--bias-audit.md
  - memory/decisions/agent-01-step-back-analyzer--failure-modes-to-avoid.md
downstream_consumers:
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-39-discussion-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
tags: scope-of-validity, case-studies, exploratory-N1, paste-ready, limitations
---

# Scope of Validity — Paste-Ready Per-Case Statements

## 0. Purpose

One paragraph per case. Each paragraph says, blunt and in order:
**what the case DOES support**, then **what it does NOT support**.
These are the sentences §6 and §8 writers paste verbatim so that
the scope walls are identical across the manuscript and no §7
Discussion sentence can quietly widen the claim.

These statements embed Bias-05 (single subject), Bias-06 (single
encoder, Case 3), Bias-08 (self-reported), Bias-09 (benchmark
monoculture), FM-04 (measured / projected), FM-05 (single-subject
trap), and V11 / V18 / V22 / R3 / R4 hedges from agent-02.

---

## Case 1 — Shakespeare stylistic LoRA

> *"Case 1 supports the claim that a 3-layer enforcement stack
> (SFT + DPO with a TCT geometric-proximity term as Layer 1,
> constrained decoding as Layer 2, and a per-sentence constellation
> guard as Layer 3) can be implemented end-to-end on Qwen3-8B + LoRA
> over 7,908 cleaned SFT examples and 12,567 DPO contrastive pairs
> drawn from the canonical Shakespeare corpus, that a 15-test
> forensic smoke verification battery (F1–F15) passes against this
> pipeline, and that a training-data echo-bug discovered during
> development can be diagnosed and fixed pre-training. The case
> does NOT support any claim about absolute Shakespearean-style
> quality, stylistic generalisation beyond Shakespeare, cross-corpus
> transfer, or superiority over alternative stylistic-control
> methods; the end-to-end stylistic cosine measurements (mean, 10th
> percentile, pass-rate at calibrated $\tau_{\text{text-style}}$)
> are PENDING T1 at submission time and will be reported with an
> explicit guard-encoder caveat (E1 / e5-large-v2 only, cross-
> encoder check deferred) and a held-out-N disclosure (tens of
> prompts, exploratory confidence interval only). Case 1 is a
> training-pipeline-integrity demonstration on a single corpus with
> a single guard encoder, not a population-level study of stylistic
> transfer."*

---

## Case 2 — Santa identity-locked talking-head clone

> *"Case 2 supports the claim that an EchoMimicV3 + LoRA pipeline
> (rank 256, alpha 512, 8 target modules with `q_audio` excluded per
> forensic audit) trained with mouth-weighted flow-matching loss
> (mouth 10×, face 2×, background 1×) runs end-to-end on 2,362
> curated clips extracted by ClipCannon's 23-stage DAG from a 16 min
> 15 s single-subject interview, that a seven-modality Santa
> constellation can be constructed as a 3,146-sample × 4,044-dim
> NPZ artefact (visual 1,725 / emotion 343 / speaker 300 / prosody
> 188 / sentiment 202 / semantic 200 / voice 188) with double L2-
> normalisation and provenance records, that YOLOv5-Face 5-point
> landmarks plus WavLM-large diarisation at cosine-distance 0.7
> plus CMU-viseme sequencing yield a reproducible curation pipeline,
> and that two pre-training forensic bugs (flow-matching sign
> convention, mouth-bbox-from-nose error) can be diagnosed and
> fixed. The case does NOT support any cross-subject, cross-session,
> cross-language, cross-microphone, cross-camera, or cross-condition
> generalisation claim; the single-subject single-session single-
> camera single-microphone nature of the source is a structural
> scope wall, not an oversight. End-to-end per-modality cosines,
> four-modality conjunction accept-rate at the operating thresholds
> (visual 0.70 / emotion 0.60 / speaker 0.80 / voice 0.85), SyncNet
> LSE-D, FID on held-out visual, and subjective naturalness are all
> PENDING T1 at submission time; the multi-modal conjunction-
> strictness claim (H2b) is derivational under Axiom 2 and is not
> empirically validated at $M > 1$ in this manuscript."*

---

## Case 3 — Voice clone beyond 0.95 WavLM SECS

> *"Case 3 supports the claim that a 50-clip reference centroid
> (selected as the top-50 by WavLM cosine similarity to a real-
> microphone recording out of 489 candidate clips derived from the
> subject's 16 min video via Demucs separation and silence-boundary
> splitting, each clip $\geq 3$ s and SNR $\geq 20$ dB) combined
> with Qwen3-TTS Full ICL and a WavLM-base-plus-sv-scored best-of-
> 12 selection at temperature 0.3 produces a mean WavLM cross-
> encoder SECS of 0.961 (max 0.975; 9/10 sentences $\geq 0.95$)
> across 10 unseen English sentences by one speaker (the author)
> recorded on one microphone in one session, with DNSMOS P.808 at
> 3.93 (parity with real recording 3.93) and UTMOS at 3.012 (parity
> with real recording 2.997) on the same recordings. The result
> does NOT generalise to other speakers, other languages (only
> English was evaluated), other dialects, other recording conditions
> (clean-audio author-read only; no LibriSpeech / VoxCeleb / cross-
> accent / cross-age-group diversity), other microphones, other
> sessions (generator-input session ≠ held-out session for the
> reference centroid, but held-out session = real-mic ground-truth
> session — a cross-session + same-session hybrid scoped in §8
> L-04), or to cross-encoder evaluation. The 0.961 headline is an
> encoder-matched measurement: WavLM-base-plus-sv is both the guard
> encoder used for best-of-12 selection and the evaluator used to
> report SECS, so the number demonstrates the guard behaves as
> specified and does not by itself constitute cross-encoder identity
> verification; an ECAPA-TDNN or Resemblyzer replication on the
> same held-out set is named future work (W1 remediation; §8
> L-01). The case is an exploratory N = 1 subject, N = 10 sentence
> demonstration by the author on author-built pipelines using
> open-source scorers; it has not been independently replicated at
> submission time, and third-party replication is a post-submission
> task."*

---

## Cross-case summary (one sentence each)

- **Case 1**: training-pipeline integrity on 7,908 + 12,567
  Shakespeare examples with 15/15 FSV passing; end-to-end quality
  numbers PENDING.
- **Case 2**: multi-modal training-pipeline integrity on a 2,362-clip
  single-subject interview with a 3,146-sample seven-modality
  constellation; end-to-end quality numbers PENDING.
- **Case 3**: encoder-matched measurement of 0.961 mean WavLM SECS
  on 10 English sentences by the author with cross-encoder
  verification deferred.

## What agent-37 / agent-38 / agent-39 / agent-40 must do

- Paste the three paragraphs above into §6.1 / §6.2 / §6.3 openers.
- Paste them again in abbreviated form in §8 Limitations as the
  scope-wall anchors.
- §7 Discussion must NOT widen any of these three scopes.
  Agent-42 adversarial reviewer regex-scans for cross-subject or
  cross-encoder claims attached to Case 3's headline.

## Confidence

**HIGH** that these three paragraphs are the correct scope
statements given:

- Bias-05 / Bias-06 / Bias-08 / Bias-09 audit (agent-14).
- FM-04 / FM-05 failure-mode discipline (agent-01).
- Case-study protocols and PENDING flags (agent-25).
- Weakness register W1 / W2 / W3 / W4 / W7 and remediation plan
  (agent-24 / agent-25).

**HIGH** that paste-ready means the sentences are defensible against
a CRITICAL adversarial review. Reviewers may ask follow-ups
(cross-encoder numbers, pre-selection distribution, $n_m$ curve,
multi-modal measurement) but cannot point to overclaim.

## Changelog

- 2026-04-16 — Initial issue by agent-26-sampling-strategist.
