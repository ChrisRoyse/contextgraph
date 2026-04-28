---
namespace: handoff
agent_id: agent-27-instrument-developer
timestamp: 2026-04-16T16:30:00Z
status: active
tags: handoff, agent-28, analysis-planner, instruments-ready
---

# Handoff: Agent-27 (instrument-developer) → Agent-28 (analysis-planner)

## Read these files FIRST, in this order

1. `./memory/decisions/agent-27-instrument-developer--instrument-specifications.md` —
   LOCKED per-instrument specs (paste-ready for §5 Methods). Contains
   A.1–A.3 (speaker-verification), B.1–B.2 (naturalness),
   C.1–C.2 (visual-identity + FLAME action-unit vocab),
   D.1–D.2 (text-style STYLE_CENTROID + vocabulary prefilter),
   E.1 (LSE-D / LSE-C pending), F.1 (SHA-256), G.1–G.3 (screening
   gates), H (JSON manifest schema), I (instrument-to-metric
   crosswalk).
2. `./memory/patterns/agent-27-instrument-developer--instrument-to-case-map.md` —
   forward map (per case) + reverse map (per instrument) +
   canonical phrasing for writers.
3. `./memory/discoveries/agent-27-instrument-developer--instrument-limitations.md` —
   §8 paste-ready limitations register (L-SECS-A/B/C,
   L-ECAPA-A/B/C, L-Qwen3-A/B, L-DNSMOS-A/B/C, L-UTMOS-A/B/C,
   L-SigLIP-A/B/C, L-FLAME-A/B/C, L-STYLE-A/B/C, L-VOCAB-A/B/C,
   L-LSE-A/B/C, L-SHA-A/B).
4. `./memory/decisions/agent-26-sampling-strategist--sampling-protocols.md` —
   populations and disjointness (already read by agent-27).
5. `./memory/decisions/agent-25-method-designer--case-study-protocols.md` —
   measurement protocols and PENDING-flag discipline.

## What agent-27 did (summary)

- Locked specifications for **15 measurement instruments** used across
  the three case studies, with canonical preprocessing, model repos,
  citations, output formats, units, and operating thresholds.
- Identified **32 distinct limitation codes** (L-SECS, L-ECAPA,
  L-Qwen3, L-DNSMOS, L-UTMOS, L-SigLIP, L-FLAME, L-STYLE, L-VOCAB,
  L-LSE, L-SHA) with paste-ready §8 disclosure sentences.
- Produced a **reproducibility JSON manifest schema** (§H of
  instrument specs) that wraps every run with embedder hashes,
  centroid hashes, calibration distribution, per-output cosine
  traces, and derived metrics.
- Built an **instrument-to-case crosswalk** in both directions.
- Flagged the **single most important §8 disclosure**: L-SECS-A
  (WavLM is both Case-3 guard and evaluator → self-consistency,
  not cross-encoder verification).
- Flagged **open carries** for agent-28 / agent-37: Case-2 emotion
  encoder and Case-2 voice-identity encoder model-IDs are not yet
  nailed down upstream; agent-37 must resolve before §6.2 final.

## What agent-28 (analysis-planner) must design

Agent-27 has specified **what** is measured and **how** the
instruments produce raw numbers. Agent-28 designs **how those raw
numbers are turned into the §6 results tables and §7 discussion
claims**. Specifically:

### 1. Per-case analysis plan

| Case | Raw outputs from agent-27 instruments | Analysis agent-28 must specify |
|---|---|---|
| 1 Shakespeare | Per-prompt, per-embedder cosines to STYLE_CENTROID; vocabulary-density ratios; regeneration counts | Mean / median / 10th-percentile per embedder; per-embedder CV; conjunction accept-rate; LoRA-only vs. full-stack delta (W2 analogue); cross-encoder agreement (W1 analogue) |
| 2 Santa | Per-scene per-modality cosines (global visual, speaker, emotion, voice, caption); conjunction accept-rate at operating τ; SyncNet LSE-D (pending); FID (pending) | Per-modality mean / median / threshold-pass-rate; four-modality conjunction accept-rate; $\lambda_m$-sweep ablations; mouth-weighted vs. uniform flow-matching ablation |
| 3 Voice | 10-sentence × 12-candidate × 3-scorer matrix (120 WavLM SECS, 120 ECAPA SECS, 10 DNSMOS, 10 UTMOS, plus 40 $n_m$-curve values); bootstrap CI | Headline table (0.961 / 0.975 / parity); pre-selection distribution histogram + TCT-delta; $n_m$-curve plot; cross-encoder agreement table; bootstrap CI |

### 2. Statistical machinery

Agent-28 must specify:

- **Bootstrap procedure** (non-parametric, percentile or BCa; 10,000
  resamples) for the N = 10 CI on Case 3 mean SECS. Agent-27 has
  pre-registered the instrument output (per-sentence vector of 10
  SECS values); agent-28 writes the resampling code.
- **Cross-encoder agreement statistics**: Pearson and Spearman
  correlation between WavLM SECS and ECAPA SECS over 10 sentences;
  Bland-Altman plot of ΔSECS for L-ECAPA-A disclosure.
- **Per-embedder CV reporting** for Case 1 STYLE_CENTROID: coefficient
  of variation across the calibration set, reported per embedder.
- **Histogram bin choice** for Case 3 pre-selection distribution
  (12 × 10 = 120 SECS values); Freedman-Diaconis rule recommended.
- **Conjunction accept-rate CIs** for Case 2: Wilson or Clopper-
  Pearson interval on the four-modality conjunction (with N = 20–100
  held-out).
- **Multiplicity correction**: Case 1 reports per-embedder thresholds
  over 9 embedders; if any per-embedder claim is made (e.g., "E10
  achieves τ ≥ X"), agent-28 must specify FWER / FDR control
  (Bonferroni or BH); agent-27's preference is to report the
  conjunction metric as primary and per-embedder as descriptive,
  sidestepping multiple-testing.

### 3. Tables and figures agent-28 should plan for §6

- **Table 6.1 Case 1**: per-embedder τ, calibration-set CV, mean
  held-out cosine, threshold pass-rate, LoRA-only vs. full-stack
  delta (all T1 PENDING).
- **Table 6.2 Case 2**: per-modality τ, mean / median cosine,
  threshold pass-rate, conjunction accept-rate (T3 PROJECTED; T1
  PENDING).
- **Table 6.3 Case 3**: mean / max SECS; DNSMOS / UTMOS parity;
  bootstrap CI; cross-encoder agreement; pre-selection
  distribution summary; $n_m$-curve — **headline T1 MEASURED**,
  auxiliary W1 / W2 / W3 PENDING.
- **Figure 6.1**: pre-selection SECS distribution histogram (120
  candidates), pre-selection mean vs. post-selection (argmax) mean,
  with TCT-delta annotated.
- **Figure 6.2**: $n_m$-curve for Case 3 ($n \in \{10, 25, 50, 100\}$).
- **Figure 6.3 (Case 1)**: per-embedder cosine histograms across
  calibration set with τ_j annotated.
- **Figure 6.4 (Case 2)**: four-modality cosine scatter matrix (T3
  PROJECTED placeholder).

### 4. Provenance / reproducibility check-plan

Agent-28 must specify **which checks agent-43 (reproducibility-checker)
runs** against agent-27's JSON manifest schema. At minimum:

- Manifest completeness: every required field non-null.
- Hash validation: `centroid_hash` matches the SHA-256 of the NPY
  artefact; `embedder_version_hash` matches the downloaded weight
  SHA-256.
- Threshold lock: `threshold_set` unchanged from calibration to
  evaluation (hash-compared).
- Per-output trace length == held-out sample size.
- Frozen-encoder audit: agent-25's TCT §2.3 hook outputs
  `all_phi_requires_grad_false = True`.

### 5. Writer contracts agent-28 hands to agent-37 / agent-38

- Agent-37 (methodology-writer) gets from agent-27:
  - Section-5-ready instrument paragraphs (A.1–F.1).
  - Canonical phrasing register (instrument-to-case map §3).
  - Screening-gate specs (G.1–G.3) for §5.4 methods subsection.
- Agent-38 (results-writer) gets from agent-27 + agent-28:
  - Table / figure skeletons (above).
  - Tier flags per number (T1 MEASURED / T1 PENDING / T3 PROJECTED).
- Agent-40 (limitations-writer) gets from agent-27:
  - The 32 limitation codes, each with a paste-ready sentence.
  - Priority ordering: L-SECS-A first.

## Open items agent-27 flags for agent-28

1. **Case-2 emotion encoder**: agent-22 / agent-26 reference an
   "emotion encoder" but the specific HuggingFace model ID is not
   locked. Candidates include `j-hartmann/emotion-english-distilroberta-base`
   (text), `ehcalabres/wav2vec2-lg-xlsr-en-speech-emotion-recognition`
   (speech), or a proprietary audio emotion encoder. Agent-28 should
   either pick one and add a corresponding limitation disclosure or
   mark the Case-2 emotion modality as "encoder TBD" in §6.2.
2. **Case-2 voice-identity encoder**: similarly referenced without a
   specific model-ID. WavLM (A.1) is a reasonable default, but then
   voice and speaker modalities would share an encoder, collapsing
   conjunction strictness. Agent-28 should recommend either
   (i) WavLM for speaker + a distinct voice-identity encoder
   (e.g., `pyannote/embedding` or Resemblyzer for voice-print), or
   (ii) merging speaker and voice into a single modality with one
   threshold and documenting the simplification.
3. **Case-3 pre-selection distribution**: the 120-candidate SECS
   matrix has not yet been run — agent-27's instrument is ready; a
   post-submission run-plan is needed from agent-28.
4. **Case-3 $n_m$-curve**: similarly ready, run pending. Agent-28
   specifies whether this is in-scope for the arXiv submission or
   deferred to v2.
5. **Human-Likert protocol**: agent-26's handoff item 4 asks whether
   3-rater Likert scoring is in-scope. Agent-27's recommendation
   (as instrument developer): **out of scope for arXiv v1**; Likert
   requires blinded rater recruitment and IRB-equivalent process
   that cannot close by submission. Agent-28 should confirm.

## Carried hedges (unchanged)

- V4 encoder-naming (every SECS / MOS mention names the encoder).
- V12 same-session-band framing on Case 3.
- V22 projected-number tags on Case 1 / 2 PENDING numbers.
- V18 frozen-encoder discipline (audit hooks in §2.3 of TCT
  protocol).
- Bias-05 single-subject (Cases 2 / 3).
- Bias-06 single-encoder (Case 3).
- Bias-08 self-reporting.
- Bias-09 benchmark-monoculture.
- FM-04 measured / projected discipline.
- FM-05 single-subject scope on §6.3.

## Dependencies agent-28 pulls forward

- Agent-27 instrument specifications + case map + limitations (this
  handoff).
- Agent-26 sampling protocols + scope-of-validity + disjointness
  matrix.
- Agent-25 TCT protocol + case-study protocols + weakness
  remediation.
- Agent-23 hypothesis ledger (analysis must connect each result to
  H-numbers).
- Agent-24 weakness register (W1 / W2 / W3 / W4 remediation hooks).
- Agent-09 standard-metrics inventory (per-cluster canonical
  phrasing).

## One-paragraph summary for agent-28

> Agent-27 has locked 15 measurement-instrument specifications
> covering identity verification (WavLM SECS A.1, ECAPA-TDNN SECS
> A.2, Qwen3-TTS internal speaker A.3 documentation-only),
> naturalness prediction (DNSMOS P.835 B.1, UTMOS B.2), visual
> identity (SigLIP-SO400M C.1, FLAME 100-D action-unit vocab C.2
> NOT-the-guard), text-style (STYLE_CENTROID 9-embedder D.1,
> vocabulary-density prefilter D.2), lip-sync (LSE-D / LSE-C E.1
> PENDING), and perceptual-hash provenance (SHA-256 F.1). Every
> instrument has a model-ID, citation, preprocessing recipe, output
> format, operating threshold (where applicable), and a set of
> paste-ready §8 limitation codes. The single most important
> disclosure is L-SECS-A (Case 3 WavLM is guard AND evaluator →
> self-consistency not cross-encoder). Cross-encoder remediation
> (ECAPA-TDNN, ColBERT / BGE) is instrument-ready, run pending.
> Pre-selection distribution (W2) and $n_m$-curve (W3) are also
> instrument-ready, run pending. Case-2 emotion and voice-identity
> encoder model-IDs are flagged as open carries for agent-28 /
> agent-37. Agent-28 now designs the statistical analysis plan,
> tables / figures, and reproducibility-check plan that turn the
> raw instrument outputs into §6 results and §7 discussion claims.

## Quality-gate status delivered by agent-27

| Gate | Status |
|---|---|
| Per-instrument model-ID + citation + weight-hash policy | **PASS** |
| Per-instrument preprocessing locked byte-level | **PASS** |
| Per-instrument output format + units + direction | **PASS** |
| Per-instrument operating threshold (where applicable) | **PASS** |
| Instrument-to-case crosswalk (forward + reverse) | **PASS** |
| §8 limitation register (32 codes, paste-ready) | **PASS** |
| Reproducibility JSON manifest schema | **PASS** |
| Screening-gate operationalisations (§E → G.1–G.3) | **PASS** |
| L-SECS-A top-priority disclosure identified | **PASS** |
| Open carries flagged (Case-2 emotion / voice encoders) | **PASS (flagged, not resolved)** |

## Confidence

**HIGH** that agent-28 can design the analysis plan and
reproducibility-check plan from these three artefacts without
re-reading the upstream research paper.

**HIGH** that the JSON manifest schema is complete enough for agent-43
(reproducibility-checker) to verify every Case-3 headline number
byte-identically from a re-run.

**MEDIUM** on Case 2 — two modalities (emotion, voice-identity) have
encoder model-IDs as open carries. Agent-28 resolution is
recommended; alternatively, agent-37 marks them "encoder TBD" in §6.2.

## Changelog

- 2026-04-16 — Initial handoff by agent-27-instrument-developer.
