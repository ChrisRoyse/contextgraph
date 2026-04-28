---
namespace: handoff
agent_id: agent-26-sampling-strategist
timestamp: 2026-04-16T09:25:00Z
status: active
tags: handoff, agent-27, instrument-developer, sampling-ready
---

# Handoff: Agent-26 (sampling-strategist) → Agent-27 (instrument-developer)

## Read these files FIRST, in this order

1. `./memory/decisions/agent-26-sampling-strategist--sampling-protocols.md` —
   LOCKED per-case sampling (Part A) and TCT reference-set
   construction (Part B); disjointness matrix (§D);
   eligibility-screening gates (§E).
2. `./memory/discoveries/agent-26-sampling-strategist--scope-of-validity.md` —
   paste-ready scope statements for §6.1 / §6.2 / §6.3 and §8.
3. `./memory/decisions/agent-25-method-designer--tct-protocol.md` —
   Phase 1 / 2 / 3 specification; calibration procedure (§3.3).
4. `./memory/decisions/agent-25-method-designer--case-study-protocols.md` —
   measurement protocols, operating thresholds, PENDING-flag discipline.
5. `./memory/patterns/agent-25-method-designer--weakness-remediation.md` —
   W1–W10 remediation-or-disclose plan (what instruments W1 and W3
   remediation require).
6. `./memory/discoveries/agent-14-bias-detector--bias-audit.md` —
   single-subject / single-encoder disclosures.

## What agent-26 did (summary)

- Locked per-case sampling (populations, procedures, N, stratification,
  held-out sets, power framing).
- Locked TCT reference-set construction including the Santa
  constellation NPZ counts (1,725 / 343 / 300 / 188 / 202 / 200 / 188;
  3,146 total × 4,044 dim) and the 50-clip Case-3 centroid.
- Resolved **W4 session audit**:
  - Case 3 generator input is **cross-session** (reference-centroid
    session ≠ held-out session, same subject).
  - Case 3 parity comparator is **same-session** (held-out session =
    real-mic ground-truth session).
  - Case 2 is **same-session** across all subsets (single 16 min
    interview) — structural feature, disclosed.
- Wrote paste-ready scope-of-validity paragraphs for all three
  cases (DOES and DOES NOT support).
- Stated explicitly that Cases 1 / 2 / 3 are **exploratory
  single-subject demonstrations, not power-calculated studies** — the
  mandatory framing §8 Limitations must carry.
- Specified eligibility gates and the disjointness matrix.

## What agent-27 (instrument-developer) must design

### 1. Measurement instruments per case

Agent-26's sampling plan specifies **which samples**; agent-27
specifies **with which instruments those samples are measured**. Per
case:

| Case | Core instruments required | Agent-26 sampling anchor |
|---|---|---|
| 1 Shakespeare | (a) E1 (e5-large-v2) text-style cosine on held-out prompts; (b) threshold-calibration script ($\tau_{\text{text-style}}$ via 10th-percentile of $S_{\text{text-style}}$ cosine distribution per TCT §3.3); (c) LoRA-only ablation scorer (pre-Layer-2 cosine); (d) cross-encoder scorer for W1 remediation (ColBERT / BGE-reranker); (e) optional 3-rater human Likert for Shakespearean-ness (50 prompts). | Sampling-protocols.md §A.1 / §E.1 |
| 2 Santa | (a) per-modality cosine scorers ($c_{\text{visual}}, c_{\text{emotion}}, c_{\text{speaker}}, c_{\text{voice}}$) on held-out scenes/prompts; (b) four-modality conjunction accept-rate at operating thresholds (0.70 / 0.60 / 0.80 / 0.85); (c) SyncNet LSE-D; (d) FID on held-out visual pool; (e) optional 3-rater naturalness Likert. | Sampling-protocols.md §A.2 / §E.2 / §B |
| 3 Voice | (a) WavLM-base-plus-sv SECS scorer (guard-identical); (b) DNSMOS P.808 scorer; (c) UTMOS scorer; (d) pre-selection distribution recorder (all 12 candidates × 10 sentences = 120 SECS values for W2 remediation); (e) ECAPA-TDNN scorer and Resemblyzer scorer for W1 remediation (cross-encoder); (f) $n_m$-curve driver over $n \in \{10, 25, 50, 100\}$ using the top-$n$ WavLM-similar clips for W3 remediation; (g) bootstrap-CI computer on N = 10. | Sampling-protocols.md §A.3 / §E.3 |

### 2. Screening instruments

The eligibility gates in §E of sampling-protocols.md are the
specification; agent-27 operationalises them:

- **Case 1**: Folger / Gutenberg canonical-passage validator; post-
  cleaning non-empty check; FSV F5 duplicate-scan runner.
- **Case 2**: YOLOv5-Face confidence thresholder; WavLM-diarisation
  runner; SyncNet LSE-D baseline; `prosody_segments` filter runner;
  scene-disjointness checker.
- **Case 3**: SNR estimator ($\geq 20$ dB); length gate ($\geq 3$ s);
  single-speaker detector; lexical overlap checker (exact-string +
  lemma) between held-out sentences and reference-clip transcripts.

### 3. Provenance instruments

TCT §1.5 / §3.5 require a provenance record attached to every
$c_m$ and every accepted output. Agent-27 must produce:

- `embedder_version_hash` computer (SHA-256 over model weights).
- `reference_set_manifest` writer (list of sample IDs + SHA-256
  content hashes).
- `centroid_hash` computer.
- `threshold_set` logger.
- Per-output cosine-trace logger (strict conjunction + relaxed
  best-of-$K$).

### 4. Reproducibility manifest

Per TCT §5, one structured JSON manifest per run containing:

- Reference-set manifest
- Embedder hashes
- Centroid hashes + construction timestamps
- Training hyperparameters table (where applicable)
- Training log (per-epoch loss, per-modality cosine stats)
- Calibration-set cosine distribution and chosen $\tau_m$
- Rejection policy and $K$
- Per-held-out-sample cosine trace

## Open items for agent-27 (must resolve)

1. **Cross-encoder speaker scorers for W1 remediation**: ECAPA-TDNN
   (SpeechBrain) and Resemblyzer wrappers. Agent-27 is empowered to
   treat these as "instrument ready, run pending".
2. **Case 1 LoRA-only ablation hook**: instrument to evaluate cosine
   against $c_{\text{text-style}}$ at the pre-Layer-2 checkpoint (W2
   analogue for Case 1).
3. **Bootstrap-CI computer for N = 10**: non-parametric bootstrap on
   the per-sentence SECS vector for Case 3 (reports $0.961 \pm 0.012$
   at 95 % CI given observed $\sigma \approx 0.02$).
4. **Human-Likert protocol (optional)**: 3 blinded raters, 50
   prompts, Likert 1–5 Shakespearean-ness (Case 1); 3 blinded raters,
   50 scenes, Likert 1–5 naturalness (Case 2). Agent-27 should
   decide whether these are in-scope or future-work.

## Carried hedges (carry forward unchanged)

- Bias-05 single-subject on Cases 2 and 3.
- Bias-06 single-encoder on Case 3.
- Bias-08 self-reporting across all cases.
- Bias-09 benchmark-monoculture on Case 3.
- FM-04 measured / projected discipline on every number.
- FM-05 single-subject scope on §6.3 opener.
- V11 / V18 / V22 / R3 / R4 hedge vocabulary.
- MP-12 composition-not-invention.

## Session audit resolved (W4)

**Case 3**: cross-session generator input, same-session parity
comparator. §6.3 opener and §8 L-04 language is paste-ready in
sampling-protocols.md §A.3.5 and scope-of-validity.md Case 3.

**Case 2**: same-session across all subsets (documented structural
feature). §6.2 opener and §8 L-04-equivalent language is paste-ready
in scope-of-validity.md Case 2.

## Dependencies agent-27 pulls forward

- Agent-26 sampling protocols (this handoff).
- Agent-25 TCT protocol + case-study protocols + weakness
  remediation plan.
- Agent-23 hypothesis ledger.
- Agent-24 weakness register.
- Agent-22 master architecture.
- Agent-14 bias audit.

## One-paragraph summary for agent-27

> Agent-26 has locked per-case sampling at (Case 1) 7,908 SFT + 12,567
> DPO Shakespeare examples with a 50–100-passage text-style
> reference set and a 20–50 held-out prompt target; (Case 2) 2,362
> curated clips from a 16 min 15 s single-subject interview with a
> seven-modality Santa constellation NPZ of 3,146 samples × 4,044
> dim (visual 1,725 / emotion 343 / speaker 300 / prosody 188 /
> sentiment 202 / semantic 200 / voice 188) and a 20–100 held-out
> scene / prompt target; (Case 3) 50-clip WavLM-ranked centroid from
> 489 candidate clips, 2 real-mic ICL references of 8–9 s each, and
> 10 held-out English sentences with verified zero lexical overlap.
> W4 session audit is resolved: Case 3 is cross-session for
> generator input and same-session for parity comparator; Case 2 is
> structurally single-session. All three cases are explicitly framed
> as exploratory single-subject demonstrations, not power-calculated
> population studies. Agent-27 must produce (a) measurement
> instruments for per-modality cosines, SECS, DNSMOS, UTMOS,
> SyncNet LSE-D, FID, and cross-encoder scorers (ECAPA-TDNN,
> Resemblyzer) for W1 remediation; (b) screening-gate runners
> implementing §E; (c) the provenance-manifest writer per TCT §1.5
> / §3.5; (d) the bootstrap-CI computer for the Case-3 N = 10
> sample; and (e) the pre-selection distribution recorder
> (12 × 10 = 120 SECS values) for W2 remediation.

## Quality-gate status delivered by agent-26

| Gate | Status |
|---|---|
| Per-case populations specified | **PASS** |
| Per-case sampling procedures specified | **PASS** |
| Per-case N specified (with PENDING flags where needed) | **PASS** |
| Per-case stratification specified | **PASS** |
| Held-out / evaluation sets specified | **PASS** |
| Power considerations: exploratory N = 1 framing stated | **PASS** |
| TCT reference-set construction (Santa constellation) specified | **PASS** |
| Disjointness matrix produced | **PASS** |
| W4 session audit resolved (Case 3 and Case 2) | **PASS** |
| Eligibility-screening gates specified | **PASS** |
| Scope-of-validity paragraphs paste-ready | **PASS** |
| Bias-05 / Bias-06 / Bias-08 / Bias-09 disclosures carried | **PASS** |
| No new primitives, no new citations | **PASS** |

## Confidence

**HIGH** that agent-27 has a tractable instrument-development task
with (a) per-case measurement targets, (b) screening gates to
implement, (c) provenance-manifest writers to specify, (d) W1 / W2
/ W3 remediation instruments identified as "instrument ready, run
pending", and (e) explicit scope-of-validity walls that constrain
what the instruments must measure (and what they need not attempt).

**HIGH** that the sampling foundation is stable enough for agent-37
(methodology-writer) to paste the sample counts, session audit, and
scope statements into §6 and §8 verbatim.

## Changelog

- 2026-04-16 — Initial handoff by agent-26-sampling-strategist.
