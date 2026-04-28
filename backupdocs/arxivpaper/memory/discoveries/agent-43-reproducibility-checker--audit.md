---
namespace: discoveries
agent_id: agent-43-reproducibility-checker
timestamp: 2026-04-17T12:15:00Z
status: active
locked: false
supersedes: none
depends_on:
  - memory/context/mission-brief.md
  - memory/context/constraints.md
  - memory/decisions/agent-25-method-designer--tct-protocol.md
  - memory/decisions/agent-25-method-designer--case-study-protocols.md
  - memory/decisions/agent-26-sampling-strategist--sampling-protocols.md
  - memory/decisions/agent-27-instrument-developer--instrument-specifications.md
  - memory/decisions/agent-28-analysis-planner--analysis-plan.md
  - memory/handoff/agent-42-to-43--critique-ready.md
  - memory/patterns/agent-42-adversarial-reviewer--stop-ship-and-edits.md
  - sections/04_systems.md
  - sections/06_method_tct.md
  - sections/07_case_studies.md
  - tasks/newdata/alldata.md
downstream_consumers:
  - agent-44-citation-validator
  - agent-46-coherence-checker
  - agent-47-final-assembler
tags: reproducibility, audit, ten-item-checklist, case-3-source-comparison, appendix-required
answers_questions: [reproducibility, method-completeness, source-fidelity]
---

# Reproducibility Audit — Cases 1, 2, 3 and TCT Method

## 0. Purpose and scope

Goal: determine whether an independent researcher could reproduce the
paper's claims **from the body of the manuscript (§3–§9) alone**. Where
the body is insufficient, state what is missing and whether the gap
should be closed with an appendix (§15 Reproducibility appendix, as the
constraints allow) or by in-line text edits.

Standard applied (from the SMAP constraints): **if it cannot be
reproduced from the submitted artefact, it cannot be trusted.**

Ten-item checklist applied per scope unit. Scope units audited:
1. **§5 TCT method** (generic protocol, not case-specific).
2. **§6.1 Case 1** — Shakespeare stylistic LoRA (PROJECTED).
3. **§6.2 Case 2** — Santa talking-head clone (PROJECTED).
4. **§6.3 Case 3** — Voice cloning (MEASURED; the reproducibility
   anchor; also cross-checked against the source
   `tasks/newdata/alldata.md`).

Each item rated: ✅ **Reproducible** / ⚠️ **Partially** / ❌
**Insufficient** / n/a.

A §15 Reproducibility appendix is required (see §6 of this audit).
Many of the §5 / §6.3 details live in upstream decision files
(agent-25 / 26 / 27 / 28) — the decision files are not shipped to
arXiv. The appendix must import them.

---

## 1. §5 TCT method — checklist

| # | Item | Status | Notes |
|---|---|---|---|
| 1 | Data (reference set) | ⚠️ | §5.2 names procedure but **no size recommendation** ($n_m$) in the body; Axiom 3 cites $[10, 10^2]$ in §3.3 but not repeated here |
| 2 | Models (generator) | ✅ | Cases named (Qwen3-8B, EchoMimicV3, Qwen3-TTS); LoRA rank/α/target modules in §5.3 and §5.6 |
| 3 | Embedders | ⚠️ | WavLM-base-plus-sv / E1 / SigLIP named in §5.6 but **weight SHA-256 not reported**; §5.3 invokes `hash(c_m)` but the computed value is not in the manuscript |
| 4 | Training hyperparameters | ⚠️ | §5.3 gives AdamW, lr, clip, warmup, bf16/fp32, but **no batch size**, **no total steps**, **no random seeds** for training |
| 5 | Inference hyperparameters | ✅ for Case 3 / ⚠️ for Case 1/2 | Case 3 states $K=12$, $T=0.3$, $\tau=0.95$; Case 1 gives $K=4$; Case 2 gives $K \in \{4,12\}$; temperature and top-p for generator not stated for Case 1/2 |
| 6 | Metrics | ⚠️ | §5.4 defines cosine but §5.6 names WavLM/E1/SigLIP **without version** — e.g., "WavLM-base-plus-sv" is correct repo name but **no revision/commit hash**; DNSMOS variant (P.808 vs P.835) specified in §6.3; UTMOS variant ("strong" vs "light") **not specified** in body |
| 7 | Hardware | ❌ | Body never states GPU, wall-clock, memory, or CUDA version |
| 8 | Random seeds / reproducibility | ⚠️ | Only the bootstrap seed (20260416) is named; training seeds, per-candidate generation seeds (despite Finding 6.7 paste edit) are not in body. bf16 vs fp32 determinism caveat present after STOP-1 edit |
| 9 | Code availability | ❌ | §5 and §4 do NOT state whether Context Graph / ClipCannon / Case 1 LoRA code is public. IC-6 (agent-32) names an asymmetric release posture (Case 1 byte-identical; Case 2/3 protocol-level) — but the `asymmetric release` labels do not appear in the manuscript body. STOP-5 deleted internal SMAP tags but did not add repro-level labels |
| 10 | Data availability | ❌ | No data-availability statement. No IRB/consent sentence for Case 2 (16-min subject interview) or Case 3 (same subject, same author) anywhere in §5 or §6. §Ethics and §Competing Interests are also not yet drafted |

**§5 verdict**: ⚠️ **Partially reproducible at the protocol level**. The
three-phase procedure is well-specified (sufficient for independent
re-implementation at concept level), but the weight hashes, training
seeds, hardware, code release, and data release statements are missing
from the body. The upstream decision files (`tct-protocol.md` §1.5 /
§2.4 / §3.6; `case-study-protocols.md`; `instrument-specifications.md`
§H) contain every missing field — they must be imported into a §15
reproducibility appendix, because the decision files are NOT shipped
to arXiv.

---

## 2. §6.1 Case 1 — Shakespeare stylistic LoRA (PROJECTED)

| # | Item | Status | Notes |
|---|---|---|---|
| 1 | Data | ⚠️ | 7,908 SFT cleaned + 12,567 DPO pairs cited (§6.1.2); source "Folger/Gutenberg" not named in body — only appears in `sampling-protocols.md` §A.1.1 |
| 2 | Models | ✅ | Qwen3-8B instruction-tuned + LoRA rank 16 α 32 on attention+MLP |
| 3 | Embedders | ⚠️ | E1 named for the STYLE_CENTROID; the 9-embedder panel (E1, E4, E6, E7, E8, E9, E10, E12, E13) specified ONLY in `instrument-specifications.md` §D.1 — **the body (§6.1.2) mentions "9-embedder STYLE_CENTROID" but does not list which 9** |
| 4 | Training hyperparameters | ❌ | §5.6 gives starting $\lambda_{\text{text-style}} = 0.5$ and DPO $\beta = 0.1$, but **no batch size, no learning-rate schedule, no total steps, no seeds** in the body |
| 5 | Inference hyperparameters | ⚠️ | $K=4$ regenerate budget named in §5.5; no temperature or top-p stated |
| 6 | Metrics | ⚠️ | "Conjunction accept-rate over 9 embedders" named in §6.1.4; per-embedder cosine formulas (especially MaxSim for E12, sparse dot for E6/E13) only appear in `instrument-specifications.md` §D.1 |
| 7 | Hardware | ❌ | Not stated |
| 8 | Random seeds | ❌ | Not stated |
| 9 | Code availability | ❌ | IC-6 specifies Case 1 is **byte-identical reproducibility** (LoRA training scripts + weights + cleaned corpus released), but this commitment is NOT made in §6.1 body |
| 10 | Data availability | ⚠️ | Shakespeare canon is public domain; the 12,567 DPO rejected completions (modern-English paraphrases) are author-constructed and their release status is undisclosed |

**§6.1 verdict**: ⚠️ **Partially reproducible**. Framework-level
description is present; specific training configuration is absent.
The paper's claim that Case 1 is "byte-identically reproducible on a
released LoRA stack" (IC-6) must be made explicit in §6.1.1 or §Ethics.

**Missing from body, available upstream**:
- 9-embedder panel list (agent-27 §D.1) — insert into §6.1.2.
- Pre-training corpus cleaning pipeline (agent-26 §A.1.2) — appendix.
- Full LoRA hyperparameter table (agent-25 §1.2) — appendix.
- Full ICL / SFT / DPO data pipeline (agent-26 §A.1.2) — appendix.

---

## 3. §6.2 Case 2 — Santa talking-head clone (PROJECTED)

| # | Item | Status | Notes |
|---|---|---|---|
| 1 | Data | ⚠️ | 16-min single-subject interview → 2,362 clips via ClipCannon's 23-stage DAG; per-modality sample counts present in §6.2.3 (visual 1,725; emotion 343; etc.). Missing: subject consent statement, clip-segment duration distribution |
| 2 | Models | ✅ | EchoMimicV3 + LoRA rank 256 α 512 on 8 modules (`q, k, v, o, k_audio, v_audio, k_img, v_img`) with `q_audio` excluded |
| 3 | Embedders | ⚠️ | Seven modalities listed in §6.2.1 and §5.6 with candidate encoders, but §5.6 introduces **alternatives** ("CLIP ViT-L/14 or SigLIP") for visual and "speech-emotion encoder" (unspecified) for emotion — ambiguous. The specific encoders must be pinned in the appendix |
| 4 | Training hyperparameters | ⚠️ | λ-schedule given in §5.6 for 5 of 7 modalities (speaker, global-visual, local-visual, motion, caption); $\lambda_{\text{emotion}}$ and $\lambda_{\text{voice}}$ **missing** — Finding 5.5 from agent-42 noted this. Also: no batch size, no total steps, no seed, no learning rate override |
| 5 | Inference hyperparameters | ⚠️ | $K \in \{4, 12\}$ named but decision rule between 4 vs 12 not stated; temperature and top-p not stated |
| 6 | Metrics | ⚠️ | Operating thresholds (visual 0.70, emotion 0.60, speaker 0.80, voice 0.85) but §6.2.3 mentions TWO voice thresholds (0.85) paired with ECAPA-TDNN **as the voice encoder** — yet §5.6 names Resemblyzer / pyannote-embedding as alternatives. The ECAPA-TDNN-as-voice-encoder is fixed in §6.2.3 text but not cross-referenced with §5.6 |
| 7 | Hardware | ❌ | Not stated |
| 8 | Random seeds | ❌ | Not stated |
| 9 | Code availability | ❌ | IC-6 specifies Case 2 is **protocol-level reproducibility** (code for the pipeline, not the 2,362 clips or LoRA weights on subject data) — but the **asymmetric-release label** is not in §6.2 |
| 10 | Data availability | ❌ | No consent statement; no dual-use disclosure; the 16-min interview is a real person recorded by the author. §Ethics would need to carry this |

**§6.2 verdict**: ⚠️ **Partially reproducible (protocol-level only by
design)**. The forensic narrative (flow-matching sign-bug, q_audio
exclusion, six companion fixes) is reproducible at audit-record level
but not as controlled ablations. The $\lambda_{\text{voice}}$ and
$\lambda_{\text{emotion}}$ gaps leave the loss function ambiguous.

**Critical missing items**:
- $\lambda_{\text{voice}}$ and $\lambda_{\text{emotion}}$ values
  (agent-42 Finding 5.5 confirmed).
- Commit-level pointer to `docs/inspect/08_training_audit.md` (with a
  public commit hash) — currently cited in prose only.
- Subject-consent statement (IRB-equivalent; the subject is not an
  anonymous volunteer, it is a named real person; dual-use mitigation).

---

## 4. §6.3 Case 3 — Voice cloning (MEASURED) — THE REPRODUCIBILITY ANCHOR

**This is the highest-stakes section because the headline numbers
(0.961 / 0.975 / 3.93 / 3.012) are presented as MEASURED.** I
cross-checked the section against `tasks/newdata/alldata.md` which
agent-25 §3 cites as the measurement source.

### 4.1 Ten-item checklist

| # | Item | Status | Notes |
|---|---|---|---|
| 1 | Data | ⚠️ | 489 candidate clips from "a 16-minute single-subject interview" cited; Demucs source separation + silence-boundary split named (alldata.md §[speaker corpus]). Missing from body: disk location of 489 corpus, SHA-256 manifest, lexical-overlap verification record |
| 2 | Models | ✅ | Qwen3-TTS named as inference-only; no LoRA |
| 3 | Embedders | ⚠️ | WavLM-base-plus-sv named as guard+evaluator. **Missing: weight SHA-256**. The model card ID (`microsoft/wavlm-base-plus-sv`) is present upstream (agent-27 §A.1) but not in body |
| 4 | Training hyperparameters | n/a | Inference-only mode |
| 5 | Inference hyperparameters | ✅ | $K = 12$ per sentence at $T = 0.3$; "Full ICL" = all 50 reference clips as context; τ = 0.95 |
| 6 | Metrics | ⚠️ | WavLM SECS formula stated (cosine of L2-normalised embeddings); DNSMOS variant explicitly P.808 in §6.3.2. **UTMOS variant NOT specified** — there are three variants (baseline, light, strong); alldata.md does not clarify; per agent-27 §B.2 we use "strong" but body is silent |
| 7 | Hardware | ❌ | Not stated |
| 8 | Random seeds | ⚠️ | Bootstrap seed 20260416 stated (§6.3.2); per-candidate generation seeds are described as "independent seeds" in §6.3.2 but the **seed-chain scheme is NOT named** (Finding 6.7 paste-ready edit would fix this) |
| 9 | Code availability | ❌ | Case 3 is **protocol-level only** per IC-6 — this posture is not labelled in §6.3 |
| 10 | Data availability | ❌ | The 489 clips, the 50-clip centroid, the two real-mic references, and the 10 held-out recordings are all author-owned audio of a named real person; no release statement; no data-sharing commitment |

### 4.2 Cross-check: §6.3 body vs. `tasks/newdata/alldata.md` source evidence

This is where reproducibility scrutiny becomes sharpest. I compared the
§6.3 numbers against the stated source document. **Three material
mismatches found.**

#### 4.2.1 ⚠️ DISCREPANCY ★★★ — Table 2 centroid-size ablation (§6.3.4)

**Paper Table 2 (§6.3.4)**:
| Enrollment method | WavLM mean | WavLM max |
|---|---|---|
| Single reference (1 clip) | 0.946 | — |
| 50-clip centroid (best clips) | **0.961** | 0.975 |

**alldata.md §5.5 "Enrollment Strategy"** (the cited source):
| Enrollment Method | WavLM Mean | WavLM Max |
|---|---|---|
| Single reference | **0.924** | **0.946** |
| 2-mic centroid | 0.924 | 0.946 |
| 50-clip centroid (best clips) | **0.939** | **0.950** |

**Problem**. The paper's "single reference" row uses **0.946** — but in
the source this is the **MAX of the single reference** ablation, not the
mean. And the paper's "50-clip centroid" row **shows 0.961/0.975** —
but in the source the 50-clip row of the SAME ablation table reports
**0.939/0.950**. The 0.961/0.975 values are the HEADLINE numbers from
a separate (later, larger-N) evaluation run at the same operating
point.

**Interpretation**. The paper is silently substituting the headline
run for the ablation row. The headline "+0.015 improvement" claim
(§6.3.4 last paragraph) is arithmetically consistent with the source's
0.939 − 0.924 = +0.015 (ablation-internal), but is ALSO stated as
0.961 − 0.946 = +0.015 (inconsistent mixing: headline 50-clip minus
ablation-internal 1-clip MAX). **Either way the Δ = +0.015 is correct,
but the row labels in Table 2 do not match the source numbers.**

**Severity**. HIGH. A reproducer running the ablation exactly as
described will get 0.939 mean on the 50-clip row, not 0.961 — and
will conclude the paper is wrong.

**Fix**. Two options:
- (A) Report the ablation verbatim (1-clip 0.924/0.946, 50-clip
  0.939/0.950, Δ = +0.015) as an independent sweep distinct from the
  headline run, and retain the 0.961/0.975 headline elsewhere.
- (B) Re-run the ablation with the headline-run's seed chain so that
  the 50-clip row matches 0.961 and the 1-clip row is re-measured on
  the same seed chain.

Agent-42 Finding 6.3 flags the missing max column but does not flag
this source mismatch. Escalating to CRITICAL.

#### 4.2.2 ⚠️ DISCREPANCY ★★ — Temperature-sweep-vs-headline discrepancy (agent-42 STOP-3)

**Paper §6.3.3 Table 1** at $T=0.3$: mean 0.932, max 0.963.
**alldata.md §5.4 Temperature Sweep** at $T=0.3$: mean **0.932**, max
**0.963**. **Match.**

**Paper §6.3.2 headline**: mean 0.961, max 0.975.
**alldata.md §5.1 "Primary Result"**: mean 0.961, max 0.975. **Match.**

So the headline and sweep are both traceable. But the paper §6.3.3
then claims "the 0.961 headline above is a separate run at the same
$T = 0.3$ operating point reported in the paper-level evaluation.
Both values are consistent with $T = 0.3$ being the sweep optimum."

**Problem**. The source does NOT describe these as two separate runs.
alldata.md §5.1 reports 0.961/0.975 as the primary result, and §5.4
reports a temperature sweep where $T=0.3$ produces 0.932/0.963. The
source is silent on whether these are one run or two. Agent-42 STOP-3
paste-edit rewrites the paragraph to say they ARE separate runs with
independent seed chains — **but this paste-edit asserts a claim
(independent seed chain, sweep before headline) that is NOT supported
by the cited source document**.

**Severity**. HIGH. If a reproducer asks "were the 10 held-out
sentences scored once or twice?" the paper now (post-STOP-3) says
"twice, at $T=0.3$, with different seed chains" — but there is no
source evidence for that claim.

**Fix**. Either (i) confirm with the author that two runs exist and
the seed chains are archived, then add the archive pointer to §6.3.2;
or (ii) withdraw the "two-run" framing and report the temperature
sweep at $T=0.3$ as the primary result (0.932/0.963) OR the headline
(0.961/0.975) as the temperature-sweep point — not both.

#### 4.2.3 ⚠️ AMBIGUITY ★ — N=14 parity comparison set

**Paper §6.3.2**: "DNSMOS P.808 (Reddy et al., 2022): clone 3.93
versus real 3.93 (MEASURED; descriptive parity, $N = 14$ comparison
set)". This reads as if both clone and real have N=14.

**alldata.md lines 1459–1467**:
- Real mic recording: 3.93 (P808)  — **appears to be a single
  recording**, not 14 samples.
- Clone mean (14 samples): 3.93 — the 14 applies to the CLONE ONLY.

**Problem**. The paper ambiguously attributes N=14 to the "comparison
set" without clarifying that (a) clone has 14 generations, (b) real is
either 1 recording or an unstated count. A reproducer cannot know
whether to compare mean(N=14 clone) to mean(N=14 real) or mean(N=14
clone) to mean(N=1 real).

**Severity**. MEDIUM. Affects how DNSMOS/UTMOS parity is computed and
what "parity within 3 sig-fig" actually means at N=14-vs-N=1 (the
real recording's variance is structurally absent from the comparison).

**Fix**. Body sentence should read: "DNSMOS P.808: clone **3.93
mean across 14 generations (one per sentence × 10 held-out +
4 calibration recordings)** versus real recording **3.93** (one
recording; clean-microphone session)". Symmetric for UTMOS.

### 4.3 §6.3 verdict

**CONDITIONAL ⚠️**. The measured headline (0.961 mean / 0.975 max on
N=10 held-out sentences, encoder-matched WavLM, Full-ICL +
50-clip-centroid + best-of-12 + T=0.3) is traceable to alldata.md §5.1
verbatim. The sub-table numbers for Table 1 (temperature sweep) match
the source exactly. **Table 2 does not match the source; STOP-3
narrative about "separate run / independent seed chain" is not
source-supported; N=14 attribution is ambiguous.** Three discrepancies
must be resolved before §6.3 is byte-reproducible.

---

## 5. §4 Production systems reproducibility

| # | Item | Status | Notes |
|---|---|---|---|
| 1 | System sizes | ⚠️ | "≈370,000 lines" and "≈67,585 lines" are given; the `git log` / `wc -l` commands to reproduce those counts are not specified |
| 2 | System versions | ❌ | No commit hash for Context Graph or ClipCannon; §4 implicitly refers to the working tree at the manuscript's composition date (2026-04-16) |
| 3 | Embedder list | ✅ | E1–E13 spans and special slots named in §4.1 paragraph 2 |
| 4 | Weight hashes | ❌ | No weight SHA-256 for any of E1–E13 or the 7 ClipCannon modalities in §4 body |
| 5 | Open-source status | ❌ | §4 does NOT state whether Context Graph or ClipCannon is open or closed source; IC-6 does not resolve this for the systems (only for the case-study artefacts) |
| 6 | Test counts | ⚠️ | "5,184 tests" / "58 MCP tools" are stated but not traceable to a commit |
| 7 | Data used by systems | ⚠️ | ClipCannon's 16-min source and 2,362 clips are not cross-linked to §6.2; the two are implicitly the same dataset, but the reproducibility handshake is not explicit |

**§4 verdict**: ⚠️ **Low-priority reproducibility risk**. The systems
are constructive evidence, not measurement objects — the numbers
(370K, 67K, 5,184, 58, 75, 59) are claims about the working tree at a
point in time. The lack of commit hashes makes independent
verification difficult but not critical to the scientific claims.

---

## 6. Gaps identified — what MUST be in a §15 Reproducibility appendix

Based on the ten-item checklist across §5 and §6.1–§6.3, the following
items are:
- **(A)** present in upstream decision files but **absent from the
  manuscript body**, AND
- **(B)** required for an independent reproduction attempt.

These items together constitute a §15 Reproducibility appendix.

### 6.1 Required appendix content (priority-ordered)

| # | Content | Body location referenced | Source in upstream decisions |
|---|---|---|---|
| A1 | Full per-case hyperparameter table (lr, batch, steps, seeds, precision) | §5.3, §5.6, §6.1, §6.2, §6.3 | agent-25 `tct-protocol.md` §2.4 + `case-study-protocols.md` §1–§3 |
| A2 | Embedder version-hash table (SHA-256 for all 13 Context Graph embedders + WavLM-base-plus-sv + SigLIP-SO400M + ECAPA-TDNN + DNSMOS P.808 ONNX + UTMOS) | §4.1, §5.6, §6.3 | agent-27 §A.1–§B.2 model cards + `instrument-specifications.md` §H manifest schema |
| A3 | Data-release manifest (what is released, where, under what license, with what consent posture) | §4, §6.1, §6.2, §6.3, §Ethics | IC-6 asymmetric-release posture (agent-32) + agent-31 ethics statement |
| A4 | Code-release manifest (Case 1 byte-identical; Case 2 protocol-level; Case 3 protocol-level) with repository URLs and commit pins | §6.1, §6.2, §6.3 | IC-6 posture + mission brief §"Source materials" |
| A5 | Sampling-procedure audit trail (diarisation threshold, face-detection confidence, SNR floor, viseme-curriculum weights, WeightedRandomSampler rates) | §6.2, §6.3 | agent-26 §A.1.2, §A.2.2, §A.3.2 |
| A6 | Full JSON manifest schema (agent-27 §H) and an example instantiation for Case 3 | §6.3 | agent-27 §H verbatim |
| A7 | Case 1 9-embedder panel (E1, E4, E6, E7, E8, E9, E10, E12, E13) — mentioned as "9 embedders" in §6.1 body but list not given | §6.1 | agent-27 §D.1 |
| A8 | Case 3 held-out sentence-selection criteria (the 10 sentences; lexical-overlap verification procedure; authoring process) | §6.3 | agent-26 §A.3.5 + agent-42 VT-I1 flag |
| A9 | Case 3 pipeline version manifest — `qwen3-tts-foricl-v1` currently undefined | §6.3 | not yet specified; must be created by author |
| A10 | Hardware statement (GPU model, VRAM, CUDA version, wall-clock per case, bf16/fp32 precision posture) | §5.3, §6.1, §6.2, §6.3 | alldata.md §3.15 hints at "RTX 5090 + WSL2" for ClipCannon but no statement per case |
| A11 | Case 3 source-cross-check table: §6.3 numbers ↔ alldata.md line numbers (an integrity map) | §6.3 | this audit §4.2 |
| A12 | Case 3 Three deferred measurements (ECAPA cross-encoder, pre-selection distribution, $n_m$ curve): full instrument specs so a reproducer can run them independently | §6.3.5 | agent-27 §A.2 + agent-28 §4.6–§4.8 |
| A13 | Bootstrap CI reproducibility procedure (deterministic library name, resample count, percentile method, exact 10 observed values) | §6.3.2 | agent-28 §4.3 |

### 6.2 Body-text edits required in addition to appendix

Some items MUST also be in the body — an appendix cannot fix them:

1. **§6.1.1**: add "Case 1 artefact release is byte-identical
   reproducibility: LoRA training scripts, weights, cleaned corpus
   released under open licence" (IC-6 label).
2. **§6.2.1**: add "Case 2 artefact release is protocol-level only:
   pipeline code released; the 2,362 subject clips and LoRA weights on
   subject data are withheld under the §Ethics dual-use posture"
   (IC-6 label + §Ethics crosslink).
3. **§6.3.1**: add "Case 3 artefact release is protocol-level only:
   pipeline code released; the 50-clip centroid, real-microphone
   reference recordings, and held-out 10 sentences are withheld under
   the §Ethics subject-consent posture" (IC-6 label + §Ethics
   crosslink).
4. **§6.3.4**: **RESOLVE** the Table 2 source mismatch (§4.2.1 of this
   audit). Either re-label rows to match alldata.md §5.5 exactly or
   re-run the ablation at the headline seed chain.
5. **§6.3.3**: **RESOLVE** the STOP-3 two-run claim (§4.2.2 of this
   audit) by either citing the archived second seed chain or
   withdrawing the two-run framing.
6. **§6.3.2**: **RESOLVE** the N=14 ambiguity (§4.2.3 of this audit).
7. **§4**: state open-source status of Context Graph and ClipCannon.
8. **§4.1 / §5.6**: add pinned embedder version (commit SHA-7 at
   least) for the 13 Context Graph embedders and the three Case-3
   scorers (WavLM, DNSMOS, UTMOS).

---

## 7. Cross-cutting reproducibility issues

### 7.1 Asymmetric release posture (IC-6) not labelled

Agent-32 IC-6 resolution (a central decision) specifies that the three
cases have three different reproducibility levels:
- **Case 1** = byte-identical reproducibility.
- **Case 2** = protocol-level reproducibility.
- **Case 3** = protocol-level reproducibility.

**The labels "byte-identical" and "protocol-level" appear exactly ZERO
times in `sections/07_case_studies.md`.** This is a first-order
reproducibility disclosure gap. A reader finishing §6 has no way to
know what is released.

**Fix**: three sentences (one per case, §6.1.1 / §6.2.1 / §6.3.1) as
specified in §6.2 of this audit above. Cost: 10 minutes of editing.

### 7.2 "qwen3-tts-foricl-v1" is undefined

The phrase "pipeline version `qwen3-tts-foricl-v1`" appears three
times in the manuscript (§6.3.1, §6.3.2, §6.3.4 / Table 3 caption) but
is never defined — no manifest, no commit hash, no URL, no constituent
component list.

**Fix**: either (i) define `qwen3-tts-foricl-v1` in §15 appendix as a
JSON manifest (per agent-27 §H) with all component versions pinned, or
(ii) drop the version label and cite git SHA-7 of the pipeline
repository directly.

### 7.3 Bootstrap CI determinism incomplete

The paper states: "10,000 resamples, fixed seed 20260416, percentile
method" (§6.3.2). To reproduce byte-identically, a reader also needs:
- The **library and version** (numpy, scipy, scikit-learn all have
  slightly different bootstrap implementations).
- The **exact 10 SECS values** (currently only the min 0.914, max
  0.975, and mean 0.961 are in the body; the raw 10-vector is not).
- The **CI endpoint rounding rule** (nearest 3 sig-fig; currently
  `[0.951, 0.971]` is stated with 3 sig-fig but no policy).

**Fix**: §15 appendix row A13 — state "scipy.stats.bootstrap(),
scipy version 1.12+, n_resamples=10000, method='percentile', fixed
seed 20260416", and table the 10 raw SECS values.

### 7.4 Cross-encoder measurement (W1) re-score instrument pinned but not run

Agent-27 §A.2 (ECAPA-TDNN) fully specifies the cross-encoder
remediation instrument. Agent-28 §4.6 specifies the analysis plan
(Pearson/Spearman, Bland-Altman). The instrument is re-implementable
today. **The paper flags this as "PENDING" (§6.3.5 item 1) with
justification "requires an independent model weight audit for
licensing and version-reproducibility" (STOP-7 paste-edit
justification). That justification is weak** — SpeechBrain's
`speechbrain/spkrec-ecapa-voxceleb` is licensed Apache-2.0 and
version-pinnable by HuggingFace revision hash. The true reason for
deferral appears to be time, not licensing. Recommendation: weaken
the justification to "time constraints of v1 submission" and promise
delivery in v2.

### 7.5 Session structure (W4) well-disclosed in upstream, hidden in body

Agent-26 §A.3.5 contains a clear disclosure:
> The 50 reference clips and the 489-candidate pool come from the
> 16-min **video interview session**; the 10 held-out sentences and
> the real-microphone reference were recorded **in a separate
> session** (different day, different room, different microphone).
> DNSMOS/UTMOS parity uses the same-session real-microphone pair.

This is a structural finding that could read as a subtle selection
effect if a reviewer noticed it independently. §6.3.2 of the paper
alludes to it ("same-session verification band") but does not state
the two-session structure explicitly. **Reproducibility concern**: a
reader attempting to re-acquire the dataset cannot tell from §6.3
alone whether "the 50-clip centroid" and "the 10 held-out sentences"
come from the same or different sessions.

**Fix**: one sentence in §6.3.1 (or §Ethics):
> "Reference centroid and held-out sentences are from different
> recording sessions of the same subject; DNSMOS/UTMOS parity is
> computed against real-microphone recordings from the held-out
> session."

---

## 8. Summary — overall reproducibility posture

| Scope unit | Protocol-level reproducibility (body only) | Byte-identical reproducibility (body only) | With a §15 appendix |
|---|---|---|---|
| §5 TCT method | ⚠️ partial (missing seeds, hardware, hashes) | ❌ not possible | ✅ achievable |
| §6.1 Case 1 | ⚠️ partial (9-embedder panel not listed, hyperparams absent) | ❌ not possible without released LoRA | ✅ byte-identical (per IC-6 commitment) once appendix + release land |
| §6.2 Case 2 | ⚠️ partial (missing λ_voice, λ_emotion, encoder pins) | ❌ not possible by design (IC-6 protocol-level only) | ✅ protocol-level |
| §6.3 Case 3 | ❌ **unreproducible as written** — Table 2 source mismatch, STOP-3 unsupported claim, N=14 ambiguity | ❌ not possible by design | ⚠️ **conditional on fixing 3 mismatches** |
| §4 Systems | ⚠️ partial (no commit hashes) | ❌ not possible (no hashes) | ✅ with commit pins |

**Headline**. The paper's case-study reproducibility is **fragile at
the measurement level**. The most dangerous finding is §4.2.1: the
§6.3.4 Table 2 does not match the cited source document
(`alldata.md` §5.5). A reproducer running the enrollment-strategy
ablation will get 0.939/0.950 for the 50-clip row, not 0.961/0.975.
The +0.015 Δ is numerically defensible from the source but the row
labels and absolute numbers in the paper are mis-attributed.

A §15 Reproducibility appendix **is required**. Without it, even the
protocol-level reproducibility for §5 and §6.1/§6.2 is blocked on
missing hashes and seeds. With the appendix + the six body-text
edits of §6.2 (this audit), the paper moves to:

- §5: **protocol-level reproducible** from body + appendix.
- §6.1: **byte-identical reproducible** once the released LoRA drops
  (IC-6 commitment).
- §6.2: **protocol-level reproducible**; numbers remain PROJECTED.
- §6.3: **byte-identical reproducible** once Table 2 / STOP-3 / N=14
  are resolved. The MEASURED headline itself (0.961 / 0.975 /
  3.93 / 3.012) is well-traceable to alldata.md §5.1.

---

## 9. Specific recommendations (ordered by priority)

### 9.1 CRITICAL — must be resolved before arXiv v1

1. **Resolve §6.3.4 Table 2 source mismatch** (§4.2.1 of this audit).
   Options: (a) re-label rows to alldata.md §5.5 numbers; (b) re-run
   ablation with the headline seed chain.
2. **Resolve §6.3.3 STOP-3 two-run claim** (§4.2.2). Either
   substantiate with archive pointer or withdraw.
3. **Resolve §6.3.2 N=14 ambiguity** (§4.2.3). State whether real
   recording is N=1 or N=14.
4. **Draft §15 Reproducibility appendix** importing items A1–A13 of
   §6.1 above.
5. **Draft §Ethics statement** carrying subject-consent for Case 2/3,
   dual-use mitigation, and the three-tier IC-6 asymmetric-release
   posture.
6. **Label case-reproducibility level in §6.1.1, §6.2.1, §6.3.1**
   ("byte-identical" / "protocol-level" per IC-6).

### 9.2 HIGH — strongly recommended

7. Add $\lambda_{\text{voice}}$ and $\lambda_{\text{emotion}}$ to §5.6
   Case 2 block (Finding 5.5).
8. Define `qwen3-tts-foricl-v1` in the appendix or drop it.
9. State open-source status of Context Graph and ClipCannon in §4.
10. List the 9 STYLE_CENTROID embedders explicitly in §6.1.2.
11. State GPU / CUDA / wall-clock in §5 or §15 appendix.
12. Name bootstrap library + version in §6.3.2.

### 9.3 MEDIUM — improves reproducibility appreciably

13. State session structure of Case 3 explicitly (§6.3.1).
14. Weaken the ECAPA-deferral justification from "licensing" to "time"
    (§6.3.5 item 1 STOP-7 paste-edit).
15. Name the UTMOS variant ("strong") in §6.3.2.
16. Add commit hash for `docs/inspect/08_training_audit.md` (§6.2.2).

### 9.4 LOW — nice to have

17. Add Folger/Gutenberg source attribution for Case 1 corpus (§6.1.2).
18. Add `git log` / `wc -l` commands in §15 appendix to reproduce the
    370K / 67,585 / 5,184 counts.

---

## 10. Checklist for authors (the paste-ready author-facing version)

Before arXiv v1 submission, confirm:

**Methods (§5)**:
- [ ] Training hyperparameters in body OR in §15 appendix: batch size,
      lr, steps, seeds, precision, hardware.
- [ ] All embedders have weight-hash references (appendix or body).
- [ ] Threshold calibration procedure stated (currently good; §5.4).
- [ ] Freeze discipline stated (currently good; §5.3).

**Materials (§6)**:
- [ ] §6.1: 9-embedder panel listed; LoRA config stated.
- [ ] §6.2: λ-schedule complete for all 7 modalities; encoders pinned.
- [ ] §6.3: Table 2 reconciled with alldata.md §5.5; STOP-3 claim
      substantiated; N=14 disambiguated.

**Data (§15)**:
- [ ] Data-availability statement drafted.
- [ ] IRB-equivalent / subject-consent posture for Case 2/3.
- [ ] De-identification for 2,362 clips and 50-clip centroid.

**Code (§15)**:
- [ ] IC-6 three-tier release posture labelled per case.
- [ ] Case 1 release DOI or GitHub URL committed.
- [ ] Case 2/3 release DOI or GitHub URL committed (protocol-level).

**Package (§15 Reproducibility appendix)**:
- [ ] A1–A13 content present.
- [ ] JSON manifest schema imported from agent-27 §H.
- [ ] 10-value raw SECS vector for Case 3 listed.
- [ ] Bootstrap CI reproducibility procedure fully specified.

---

## 11. Confidence

**HIGH** that the ten-item checklist and the three Case-3 source
mismatches (§4.2.1 / §4.2.2 / §4.2.3) are correct findings. The
alldata.md source was read in full at the relevant sections.

**HIGH** that a §15 Reproducibility appendix is REQUIRED. Without it,
even the protocol-level reproducibility for §5 and §6.1/§6.2 is
blocked by missing hashes and seeds.

**MEDIUM** that I have caught every reproducibility issue. I did NOT
verify the 5,184 / 370K / 67,585 / 58 / 75 / 59 counts against the
actual git tree; I did NOT independently re-run the bootstrap or the
temperature sweep. My audit is a specification-vs-source comparison
and a ten-item checklist, not a re-execution of the pipeline.

**HIGH** that fixing items 1–6 of §9 moves the paper from
"conditionally reproducible" to "reproducible at the scope disclosed
in IC-6". The underlying claims are defensible; the presentation of
the measurements needs tightening.

---

## 12. Changelog

- 2026-04-17 — Initial audit by agent-43-reproducibility-checker.
