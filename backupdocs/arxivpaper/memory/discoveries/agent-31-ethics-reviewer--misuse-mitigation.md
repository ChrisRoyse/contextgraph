---
namespace: discoveries
agent_id: agent-31-ethics-reviewer
timestamp: 2026-04-16T20:30:00Z
status: active
locked: false
supersedes: none
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-25-method-designer--case-study-protocols.md
  - memory/decisions/agent-26-sampling-strategist--sampling-protocols.md
  - memory/discoveries/agent-30-risk-analyst--fmea-risk-register.md
  - memory/handoff/agent-30-to-31--risks-ready.md
  - memory/decisions/agent-31-ethics-reviewer--ethics-statement.md
downstream_consumers:
  - agent-32-contradiction-analyzer
  - agent-37-methodology-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
  - agent-47-submission-packager
tags: misuse, deepfake, dual-use, responsible-disclosure, artefact-release, audit, locked
answers_questions: [Q07, Q08, Q15, Q16]
---

# Misuse-and-Mitigation Audit

## 0. Purpose

This audit answers a single question: **given what we release and
what we withhold, what specific misuse steps can a malicious actor
take from this manuscript, and what steps are they blocked from
taking?**

The frame is adversarial. The audit is written as if a hostile reader
is assembling a threat model from the paper. Each row lists a
capability the adversary wants, whether our release posture enables
or blocks it, and the residual risk.

## 1. Adversary threat model

The adversary is a moderately-resourced bad actor who:

- Has read this paper in full, including appendices.
- Has access to public Hugging Face model weights (Qwen3-TTS,
  WavLM-base-plus-sv, SigLIP, CLIP, EchoMimicV3 open checkpoints).
- Has ~1 GPU (consumer-tier, e.g., 24 GB RTX 4090) or cloud-rental
  equivalent.
- Does **not** have our Case 2 subject footage or derived clips.
- Does **not** have our author's (Case 3) voice clips or reference
  centroid.
- May or may not have target-subject data of their own.

The adversary's goal: produce an identity-locked voice or face clone
of a **chosen third-party subject** (not our subjects) at the
quality tier we demonstrate.

---

## 2. Capability audit (what the adversary can and cannot do)

### 2.1 Capability A — Clone our Case 2 subject's face / voice

| Sub-capability | Enabled by paper? | Why |
|---|---|---|
| Reproduce the Case 2 subject's likeness from paper alone | **NO** | 16-min raw interview footage is withheld; 2,362 derived clips are withheld; Santa-specific LoRA adapter weights are withheld; per-modality centroid .npz files are withheld. |
| Reproduce the Case 2 subject's likeness from paper + public images | Partially, but not via our pipeline | An adversary who independently scrapes images of the subject could train an identity-clone from scratch using any public face-cloning method. This is **not unique to our paper** — EchoMimicV3 alone suffices. Our paper does not increase this adversary's capability. |
| Clone the Case 2 subject using our Santa LoRA weights | **NO** | Weights withheld (see §Ethics asymmetric release posture). |

**Residual risk**: LOW for Case 2 subject specifically. The paper
does not add adversary capability to clone this individual beyond
what was already publicly possible.

### 2.2 Capability B — Clone the author's voice / likeness (Case 3)

| Sub-capability | Enabled by paper? | Why |
|---|---|---|
| Reproduce the author's voice from paper alone | **NO** | 489-clip speaker corpus withheld; 50-clip reference centroid withheld; two 8–9 s clean-mic references withheld. |
| Clone the author's voice from public footage | Possible but not via our artefacts | Author publishes in public; an adversary could scrape public footage and use any voice-cloning pipeline (XTTS, Bark, Qwen3-TTS directly). Our paper does not add adversary capability to clone the author beyond public-channel baseline. |
| Clone author's voice using our pipeline on scraped data | Marginal uplift | The paper describes the **protocol** (Full ICL + 50-clip centroid + best-of-12 under WavLM guard at T=0.3). An adversary who scrapes 50 author clips could apply this protocol against Qwen3-TTS public weights. The protocol is a modest uplift on what already-published work (Chen et al. 2024 VALL-E 2; Casanova et al. 2022 YourTTS) describes. |

**Residual risk**: LOW-MEDIUM. The author accepts the residual as a
structural consequence of open research publication on voice cloning.
The author's voice is already cloneable by any moderately-resourced
adversary using pre-existing public methods; our paper adds a small
quality increment (~0.05–0.10 SECS), not a capability threshold.

### 2.3 Capability C — Clone an arbitrary third-party subject using our method

| Sub-capability | Enabled by paper? | Why |
|---|---|---|
| Re-implement TCT from the manuscript | **YES** | Protocol, thresholds, and pipeline structure are released. This is the reproducibility posture by design. |
| Apply TCT to a non-consenting third-party subject | **YES, but requires adversary-supplied subject data** | The paper explicitly states TCT reduces the sample-complexity threshold from thousands to "tens of reference clips". An adversary with 50 clips of a target could, in principle, apply the Case 3 protocol. |
| Get better results than existing public voice-cloning methods | **MARGINAL** | Our Case 3 result (0.961 mean WavLM SECS, 9/10 ≥ 0.95) exceeds VALL-E 2's reported 0.881 by +0.080 within the WavLM-family measurement. Cross-encoder triangulation (ECAPA / Resemblyzer) is PENDING; cross-encoder result may be lower. The adversary's marginal quality gain from reading our paper vs. using XTTS / Bark / YourTTS is bounded. |
| Get a **per-output verifiable identity guarantee** | **YES, this is novel** | The inference-time guard (per-generation cosine predicate against a frozen centroid) is the load-bearing novel capability the paper provides. An adversary who wants not just "a voice clone" but "a voice clone with per-output identity verification" gains capability from this paper. |

**Residual risk**: MEDIUM. This is the core dual-use concern. The
method enables small-data identity cloning with per-output
verification. Mitigations below.

### 2.4 Capability D — Use TCT as a detection / defensive tool

| Sub-capability | Enabled by paper? | Why |
|---|---|---|
| Use the frozen-encoder guard as an anti-drift check on a protected pipeline | **YES** | This is the **defensive** use — the same mechanism flipped in purpose. TCT as a **runtime integrity check** for a production generator is explicitly within scope of Context Graph's constellation-guard design. |
| Build a detector that identifies encoder-matched self-consistency artefacts | **PARTIALLY** | The paper discusses (in §7 and §8 L-01) the circularity of encoder-matched evaluation; this is a pointer for detection research, not a detector itself. |

**Residual risk**: LOW on defensive use. Actively encouraged by the
paper's §7 Discussion.

---

## 3. What the paper does **not** release (explicit blocklist)

For reproducibility transparency and adversary-audit clarity, the
following artefacts are **withheld** from the manuscript's data /
code supplement:

1. Case 2 subject's 16-min raw interview footage.
2. Case 2 subject's 2,362 curated clips.
3. Case 2 subject's per-modality reference centroids (`vis_emb`,
   `emo_emb`, `spk_emb`, `pro_data`, `sent_emb`, `sem_emb`,
   `voice_emb` in `all_embeddings.npz`, 8.8 MB total).
4. Case 2 Santa-specific LoRA adapter weights.
5. Case 2 Santa-specific FLAME parametric-head fits (3,477 frames ×
   100 coefficients).
6. Case 3 (author) 489-clip speaker corpus.
7. Case 3 (author) 50-clip reference centroid.
8. Case 3 (author) two 8–9 s clean-microphone reference recordings.
9. Case 3 (author) 10 held-out sentence recordings.
10. Any synthetic artefact generated by either Case 2 or Case 3 pipeline
    that contains subject likeness or voice.

## 4. What the paper **does** release (explicit allowlist)

1. Full methodology prose (§4, §5, §6) and pseudocode / equations.
2. Protocol-level reproducibility: hyperparameters, sampling budgets
   (K=12, T=0.3, 50-clip centroid, best-of-K selection under
   WavLM-scored predicate), threshold operating values.
3. Context Graph source code (Rust workspace, ~370K LOC,
   open-source).
4. ClipCannon source code (Python package, ~67K LOC, open-source).
5. Case 1 Shakespeare LoRA training scripts against public-domain
   Shakespeare corpus.
6. Case 1 public Shakespeare corpus (Folger / Gutenberg editions,
   already public-domain).
7. Agent-27 reproducibility manifest schema (weight SHA-256, centroid
   hashes, threshold sets).
8. Weight-pin pointers to public model weights (Qwen3-TTS, WavLM,
   SigLIP, ECAPA-TDNN, DNSMOS, UTMOS) with SHA-256 hashes and Zenodo
   preservation DOIs.
9. Evaluation-framework scripts (WavLM SECS computation, DNSMOS /
   UTMOS computation against author-supplied references).

---

## 5. Specific misuse mitigations embedded in the method

### 5.1 Mitigations the paper's methodology already contains

| Mitigation | Mechanism | Status |
|---|---|---|
| Asymmetric artefact release | Protocol released; subject-specific weights + clips withheld | **In place** for this submission. |
| Encoder-matched evaluation disclosure | §6.3 explicit disclosure that the 0.961 is WavLM-vs-WavLM; cross-encoder triangulation flagged as pending | **In place** per agent-29 VT-I3 / VT-C1. |
| Per-output provenance trace | Cosine trace attached to every Case 2 / Case 3 generation (TCT Phase 3 specification) | **In pipeline**. Enables cryptographic binding of output to centroid hash in downstream deployment. |
| Centroid hash stamping | Every generated artefact can be signed with the centroid hash that accepted it | **In pipeline** via agent-27 §H manifest. |

### 5.2 Mitigations the paper recommends but does not implement

| Mitigation | Target | Implementation cost | Named in |
|---|---|---|---|
| Output watermarking at synthesis time | Detectable signal embedded in audio / video output | Medium — requires watermark scheme choice (e.g., AudioSeal for audio, Stable Signature for video diffusion) | §Ethics Responsible Disclosure; §10 Future Work |
| Cryptographic provenance metadata (C2PA-style) | Tamper-evident "this was TCT-generated with centroid hash X" metadata | Low-Medium — use C2PA / Coalition for Content Provenance standards | §Ethics Responsible Disclosure; §10 Future Work |
| Cross-encoder detection-side research | Detector trained to flag encoder-matched self-consistency artefacts | High — requires adversarial training against multiple speaker / face encoder families | §Ethics Responsible Disclosure; §10 Future Work |
| Revocation-compatible centroid binding | If a subject revokes consent, centroid is marked revoked; bound outputs can be detected | Medium — requires a registry service | §10 Future Work |

### 5.3 Mitigations the paper does NOT recommend (out of scope)

- **Weight-level obfuscation** of our released method (not useful;
  method is protocol-level and already reproducible from prose).
- **Licence-based restrictions on downstream use** of the method
  (not enforceable for ML methods in arXiv cs.LG; we do not pretend
  otherwise).
- **Voluntary government registration of identity-clone pipelines**
  (policy question; out of scope for a methods paper).

---

## 6. Reproducibility-vs-responsible-disclosure trade-off

### 6.1 The trade-off stated honestly

Full reproducibility of the **numbers** in Case 2 and Case 3 would
require releasing subject-specific weights, clips, and centroids.
Releasing those artefacts would:

- **Pro**: enable byte-identical third-party replication of the 0.961
  mean SECS and the Case 2 conjunction accept-rate.
- **Con**: enable identity cloning of the Case 2 subject and the
  author by arbitrary third parties, without their further consent
  and without revocation capability.

**We choose partial release**. Protocol is released; subject-specific
identity artefacts are not. This is **protocol-level reproducibility,
not byte-identical reproducibility** for Cases 2 and 3.

Case 1 (Shakespeare) is released in full because its subject (a
public-domain corpus) has no consent concern; this is our
**reproducibility flagship**.

### 6.2 What a replicator can do

A researcher who wants to replicate our protocol:

1. **Case 1 Shakespeare**: full byte-identical replication possible.
   Code, data, evaluation framework released; public-domain corpus is
   public.
2. **Case 2 Santa**: protocol-level replication on a replicator's own
   consented subject. ClipCannon 23-stage DAG + EchoMimicV3 + 7-modality
   reference centroid construction + mouth-weighted flow-matching LoRA
   is fully specified in §5 + §6.2 + §15 Appendix. Replicator supplies
   their own single-subject interview footage.
3. **Case 3 Voice**: protocol-level replication on a replicator's
   own consented subject. Full ICL + 50-clip centroid + best-of-12 at
   T=0.3 against WavLM SECS is fully specified. Replicator supplies
   their own voice data.

### 6.3 What the trade-off costs the paper

- Case 2 and Case 3 cannot be reproduced **numerically** by
  third parties from our artefacts alone. This is a real
  reproducibility limitation. We disclose it in §8 L-Reproducibility
  and position Case 1 Shakespeare as the reproducibility flagship
  per agent-30 R1-05 mitigation plan.
- This is accepted as the **ethically-correct posture**. A paper that
  released subject-specific weights for identity cloning would be
  rejected by modern venues regardless of scientific quality.

### 6.4 Venue expectations

**arXiv** cs.LG: accepts this posture. Protocol-level reproducibility
plus explicit withholding notice is standard for identity-cloning
work.

**NeurIPS 2024 Code of Ethics**: identity-cloning work is flagged as
high-risk; reviewers expect artefact-release restriction. Our posture
aligns.

**ICML 2025 Ethics Guidelines**: require disclosure of dual-use
potential and proportionate release restriction. Our posture aligns.

**ICLR 2025 Reproducibility Checklist**: asks "did you release code
and data?" — we answer "yes to code and public-domain data; no to
subject-specific identity artefacts with explicit rationale". This
is a standard documented-withholding answer.

---

## 7. Bright-line asks for the principal author

Before agent-47 locks submission, the principal author must confirm:

1. **[CONFIRMED by author]** Written informed consent from the Case 2
   subject is on file, dated, and covers the five scope items
   (research use; manuscript publication; case-study description;
   commercial scope; revocation mechanism). If any item is absent,
   agent-31 escalates and Case 2's inclusion is re-evaluated.
2. **[CONFIRMED by author]** No synthetic artefact of the Case 2
   subject's voice, face, or likeness has been, or will be, published
   publicly.
3. **[CONFIRMED by author]** The Teleox.ai / Leapable.ai affiliations
   are correctly described in §Competing Interests.
4. **[CONFIRMED by author]** No third-party subject data exists in
   any pipeline beyond the two declared (Case 2 subject, Case 3 author).

Agent-47 checkbox list assumes ALL FOUR are confirmed by principal
author in writing before submission.

---

## 8. Residual-risk summary

| Risk | Pre-mitigation | Post-mitigation |
|---|---|---|
| Case 2 subject cloned from paper + released artefacts | N/A (cannot occur) | Cannot occur (weights + clips withheld) |
| Case 3 author cloned from paper + released artefacts | N/A (cannot occur) | Cannot occur (weights + clips withheld); author is already publicly cloneable by pre-existing methods, not uniquely enabled here |
| Arbitrary third-party cloned using our method | HIGH (the method works) | MEDIUM (per-output verification encourages provenance binding; no weights provided; quality gain over baseline is bounded) |
| Case 2 consent challenge by subject or third party | MEDIUM | LOW (consent is documented; revocation mechanism exists) |
| Academic venue rejection on ethics grounds | HIGH at NeurIPS / ICML if undisclosed | LOW with §Ethics + asymmetric release posture |
| Commercial-affiliation COI | MEDIUM | LOW with §Competing Interests + FM-01 regex pass |

---

## 9. Confidence

**HIGH** that the asymmetric release posture (protocol released,
subject-specific artefacts withheld) is the scientifically-correct
and ethically-correct stance for this manuscript.

**HIGH** that a malicious reader gains bounded capability from this
paper relative to the pre-existing public voice / face cloning
literature; the marginal uplift is the per-output verification
primitive, not the cloning capability itself.

**HIGH** that Cases 2 and 3 are protocol-reproducible but not
byte-identical-reproducible by design, and that this is disclosed.

**HIGH** that mainstream venues (arXiv cs.LG, NeurIPS 2024 Ethics,
ICML 2025 Ethics, ICLR 2025 Reproducibility) accept this posture.

**MEDIUM** on whether future policy environments (EU AI Act
implementation, US state-level deepfake laws) will retroactively
constrain publication of identity-cloning methodology regardless of
release posture. This is out of scope for the submission-window but
is a carry for v2 and for downstream deployment by Teleox.ai /
Leapable.ai.

## Changelog

- 2026-04-16 — Initial issue by agent-31-ethics-reviewer.
