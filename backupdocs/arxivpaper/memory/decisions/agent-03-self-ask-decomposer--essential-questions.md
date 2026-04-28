---
namespace: decisions
agent_id: agent-03-self-ask-decomposer
timestamp: 2026-04-16T15:45:00Z
status: active
locked: true
supersedes: none
depends_on:
  - memory/context/mission-brief.md
  - memory/patterns/agent-01-step-back-analyzer--guiding-principles.md
  - memory/decisions/agent-01-step-back-analyzer--failure-modes-to-avoid.md
  - memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md
  - memory/patterns/agent-02-ambiguity-clarifier--vocabulary-rules.md
  - memory/discoveries/agent-02-ambiguity-clarifier--ambiguities-resolved.md
tags: self-ask, decomposition, essential-questions, arxiv, locked
---

# Essential Questions — LOCKED

The arXiv manuscript is unreadable unless these 20 questions are each answered with the
evidence tier indicated (T1–T4 per glossary #21). Every question uses canonical vocabulary;
every question names (a) the manuscript section that owns the answer, (b) the primary
evidence source, (c) the downstream agent(s) responsible, (d) the current answerability
state (ANSWERABLE / PARTIAL / GAP), and (e) dependencies on other questions.

**Coverage matrix.** Claims covered: DDA, meaning compression, TCT, sample-complexity
collapse. Case studies covered: Case 1 (Shakespeare LoRA), Case 2 (Santa identity-locked
talking head), Case 3 (voice cloning). Cross-cutting: data-wall reframing, reproducibility,
ethics, failure forensics, related-work deltas, limitations.

**Locking rule.** This question set is authoritative for downstream agents (04–48). A
question may only be added, re-worded, or removed by a coordinator-signed decision file
that supersedes this one. If a downstream agent needs a new question, file a blocker at
`./memory/blockers/`, do not silently invent one.

---

## Category A — Problem framing and motivation (Q01–Q03)

### Q01 — What exactly is "the data wall" this paper reframes, and why reframe it now?
- **Section.** §1 Introduction; re-cited §3.3.
- **Evidence source.** `docs/researchpaper/_evidence_01_data_wall.md` (Villalobos et al., 2024; Epoch AI 2024 projections; industry licensing-deal signals from `_evidence_04_enterprise_market.md`).
- **Owner agents.** agent-35 (introduction-writer), agent-36 (literature-review-writer).
- **Evidence tier.** T4 framing, T1 for cited projection numbers.
- **Answerability.** ANSWERABLE with existing evidence.
- **Depends on.** None.
- **Answer shape.** Cite Villalobos et al. for the exhaustion projection (2026–2032); cite 2024–25 industry signals (Reuters / NYT licensing deals, scaling-plateau reports) for the "why now"; state the reframing: the data wall says "we lack inputs"; DDA says "we under-utilize the inputs we have" (see Tier F glossary #26 / #27; P09 / FM-03 bounds — no AGI claim).

### Q02 — What is the model-collapse risk (Shumailov et al., 2024) and why does DDA structurally avoid it?
- **Section.** §2 Related Work (collapse paragraph); §3.3 (DDA vs. synthetic data).
- **Evidence source.** `_evidence_01_data_wall.md` (Shumailov et al. 2024, Nature).
- **Owner agents.** agent-36, agent-21 (theory-builder), agent-42 (adversarial-reviewer).
- **Evidence tier.** T4 structural argument; T1 for cited collapse result.
- **Answerability.** ANSWERABLE (distinction is definitional per glossary #1 / R01 and glossary #28).
- **Depends on.** Q01.
- **Answer shape.** Model collapse requires a recursive generator→training→generator loop. DDA is a decomposition of fixed inputs through $N$ frozen embedders; there is no generative feedback loop. Therefore the collapse theorem is silent on DDA. This sentence must appear (first use in §3.3 per rule V15).

### Q03 — What existing mitigations for the data wall does prior work already offer, and what gap remains?
- **Section.** §2 Related Work.
- **Evidence source.** `_evidence_01_data_wall.md` (scaling / Chinchilla / data-efficient training); `_evidence_04_enterprise_market.md` (licensed-corpus approach).
- **Owner agents.** agent-36, agent-06 (literature-mapper).
- **Evidence tier.** T4.
- **Answerability.** ANSWERABLE.
- **Depends on.** Q01, Q02.
- **Answer shape.** Three paths currently discussed: (1) acquire more inputs (licensing, web-scale scraping, Chinchilla-optimal compute-data rebalancing), (2) synthesize new inputs (risks model collapse per Q02), (3) data-efficient training (Chinchilla, scaling laws). DDA is a fourth path (glossary #27): decompose existing inputs along $N$ frozen embedder axes (no new inputs; no generator loop). Novelty delta stated narrowly per P07.

---

## Category B — Core construct definitions (Q04–Q06)

### Q04 — What is Derived Data Abundance (DDA), stated formally, and what is the $N + \binom{N}{2}$ derivation?
- **Section.** §3.1 (concept), §3.4 (counting).
- **Evidence source.** Glossary #1; mission brief core claim 1; R01.
- **Owner agents.** agent-04 (construct-definer), agent-21.
- **Evidence tier.** T4 theoretical; T2 constructive for "N=13 → 91 signals" in Context Graph.
- **Answerability.** ANSWERABLE.
- **Depends on.** Q02 (distinction from synthetic data must already be on record).
- **Answer shape.** State the definition from glossary #1 verbatim: a fixed corpus of $n$ raw inputs through $N$ frozen independently-trained embedders yields $n(N + \binom{N}{2})$ structured supervisory signals ($n$ per-embedder labeled vectors plus $n\binom{N}{2}$ pairwise cross-embedder interaction features). For Context Graph $N=13$: $13 + 78 = 91$ signals/input. The word "orthogonal" must carry the hedge from V11 ("approximately independent; strict formal orthogonality is an open question, §10").

### Q05 — What is meaning compression, and how is it distinct from bit-, weight-, and activation-compression?
- **Section.** §3.1 and §3.5 (four-category taxonomy table).
- **Evidence source.** Glossary #2; R02; `_evidence_01_data_wall.md` (TurboQuant, GPTQ, SparseGPT references for the adjacent categories).
- **Owner agents.** agent-04, agent-36, agent-21.
- **Evidence tier.** T4.
- **Answerability.** ANSWERABLE with existing evidence.
- **Depends on.** Q04.
- **Answer shape.** Build the 4-row table: (i) bit compression — bits/bits (gzip / PNG / FLAC); (ii) weight compression — accuracy/parameters (GPTQ, SparseGPT, distillation); (iii) activation compression — memory/output quality (TurboQuant, KV-cache quantization); (iv) meaning compression — labeled signals/unit raw data (DDA). Unit columns are dimensionally distinct; that is the novelty delta per P07.

### Q06 — What is Teleological Constellation Training (TCT) as a training procedure, and how does its runtime guard differ from RLHF / DPO / Constitutional AI scalar-reward alignment?
- **Section.** §5 Method; re-cited §2.6 Related Work (alignment).
- **Evidence source.** Glossary #3 / #12 / #13; R03.
- **Owner agents.** agent-04, agent-37 (methodology-writer), agent-25 (method-designer).
- **Evidence tier.** T4 method spec; T2 for the constellation-guard code artifact.
- **Answerability.** ANSWERABLE.
- **Depends on.** Q04 (DDA produces the centroids), Q05 (meaning compression gives the signals).
- **Answer shape.** TCT optimizes a generative model so that its outputs fall within per-modality cosine thresholds $\tau_m$ of a frozen multi-modal centroid set (the constellation). Distance-to-centroid loss during training; per-output cosine validation at inference via the constellation guard (glossary #12). Contrast with RLHF/DPO/CAI: those optimize a learned scalar reward against preference pairs; there is no per-output geometric check at inference. First use of "alignment" in §2.6 must include V10 disambiguation clause.

---

## Category C — TCT method mechanics (Q07–Q09)

### Q07 — What is the three-phase TCT protocol (constellation construction → constellation-conditioned training → runtime constellation-guard), and what are the inputs/outputs of each phase?
- **Section.** §5.1, §5.2, §5.3.
- **Evidence source.** Prior `docs/researchpaper/` §5–§6; SITREP `tmp/SITREP_2026-04-16_135543.md` (Case 3 implements the protocol).
- **Owner agents.** agent-25, agent-37.
- **Evidence tier.** T4 for the protocol; T1 for the Case 3 instantiation.
- **Answerability.** ANSWERABLE.
- **Depends on.** Q06.
- **Answer shape.** Phase-1 inputs: reference set per modality → L2-normalised mean of L2-normalised embeddings → one centroid per (subject, modality) (glossary #7) → constellation (glossary #8). Phase-2: conditional generation loss + $\sum_m \lambda_m (1 - \cos(\phi_m(\hat{y}), c_m))$ over selected modalities. Phase-3 constellation guard: re-embed output through the same frozen embedders, compute per-modality cosines, accept iff all $\geq \tau_m$; otherwise resample / reject. Each phase has a formal algorithm block (pseudocode only — FM-06).

### Q08 — How are the per-modality thresholds $\tau_m$ calibrated, and what is the calibration sample size?
- **Section.** §5.3 (guard), §6.3 (Case 3 calibration numbers), §8 Limitations.
- **Evidence source.** Case 3 SITREP (the 50-clip reference set and WavLM same-session band 0.95–0.99); glossary #17 (same-session band is our own empirical observation, not a universal).
- **Owner agents.** agent-25, agent-29 (validity-guardian), agent-38 (results-writer).
- **Evidence tier.** T1 for Case 3 numbers; T4 for the calibration procedure.
- **Answerability.** PARTIAL. Case 3 threshold calibration is documented (same-session band on Case 3 speaker). Cases 1 and 2 thresholds are projected. Flag explicitly per V22.
- **Depends on.** Q07.
- **Answer shape.** For Case 3: measure the WavLM-SECS between pairs of the speaker's own real recordings from the same session; that empirical band defines $\tau_{\text{speaker}}$. State the hedge V12: "same-session band is an empirical observation on Case 3's speaker, not a universal value". For Cases 1 and 2 flag as projected (T3).

### Q09 — How does TCT differ from CLIP / SigLIP (multi-modal contrastive), WavLM speaker verification, and VALL-E 2 voice cloning?
- **Section.** §2 Related Work (by sub-line).
- **Evidence source.** `_evidence_02_voice_cloning.md` (VALL-E 2, WavLM SOTA); `_evidence_03_video_embedders.md` (CLIP / SigLIP); glossary #3 / R03.
- **Owner agents.** agent-36, agent-10 (theoretical-framework-analyst).
- **Evidence tier.** T4 (structural contrast); T1 for the cited comparators' numbers.
- **Answerability.** ANSWERABLE.
- **Depends on.** Q06.
- **Answer shape.** CLIP/SigLIP: contrastive alignment of paired (image, text) batches; learned joint space; no per-output runtime guard on an identity constellation. WavLM: produces the speaker embedding but does not itself generate; we use it as a frozen embedder and as the constellation-guard scorer in Case 3. VALL-E 2: reported "human parity" on LibriSpeech continuation (Chen et al., 2024) via zero-shot in-context learning, but uses a single speaker prompt with no centroid aggregation and no runtime cosine guard; we aggregate a 50-clip centroid and apply a WavLM-scored best-of-12 guard.

---

## Category D — Systems (Q10–Q11)

### Q10 — How is Context Graph instantiated (13 embedders, 59 RocksDB column families, 75 MCP tools, 5,184 tests), and why does the specific embedder set support the DDA count?
- **Section.** §4.1 Systems / Context Graph.
- **Evidence source.** Prior manuscript §4; `docs2/contextgraphspecs/` (18 files); mission brief `CLAUDE.md` "Key Architecture Facts" (13 embedders E1–E13; dims: 1024 / 512×3 temporal / 768 causal / 30k sparse / 1536 code / 1024 graph / 1024 HDC / 768 paraphrase / 768 entity / 128/token ColBERT / 30k SPLADE).
- **Owner agents.** agent-09 (methodology-scanner), agent-37, agent-38.
- **Evidence tier.** T2 constructive (cite line counts, test counts, CF counts, file paths).
- **Answerability.** ANSWERABLE.
- **Depends on.** Q04 (DDA count needs $N=13$).
- **Answer shape.** Enumerate E1–E13 per CLAUDE.md facts (different architectures, different training data, different objectives → satisfies the "independently-trained" clause of glossary #1). Cite 59 CFs, 75 MCP tools, 5,184 tests, ~370K Rust lines. Existence (T2) — not performance (T1).

### Q11 — How is ClipCannon instantiated (23-stage DAG, 7 modalities × 4,044 dim, 58 MCP tools, 12,000+ labeled training samples from 16 min source), and how does it realize DDA on a single 16-minute interview?
- **Section.** §4.2 Systems / ClipCannon; §6.2 Case 2 linkage.
- **Evidence source.** Prior manuscript §4; mission brief.
- **Owner agents.** agent-09, agent-37, agent-38.
- **Evidence tier.** T2 constructive; T1 for the 12,000+ sample count if directly attributable to the Santa-project log.
- **Answerability.** ANSWERABLE.
- **Depends on.** Q04, Q10.
- **Answer shape.** Enumerate the seven ClipCannon modalities (visual, audio-speaker, audio-voice, emotion, FLAME/expression, transcript, meta per V25). 16 min source → 2,362 curated clips → 12,000+ labeled training samples; this is DDA at the pipeline level (decomposition, not generation). Cite the 67,585-line count and 58 MCP tools as constructive evidence.

---

## Category E — Case-study measurements and projections (Q12–Q14)

### Q12 — What was measured in Case 3 voice cloning (the fully-T1 study)?
- **Section.** §6.3 Case 3 (header tagged "T1" per V20).
- **Evidence source.** SITREP `tmp/SITREP_2026-04-16_135543.md`; white paper `tasks/newdata/alldata.md`.
- **Owner agents.** agent-38, agent-43 (reproducibility-checker).
- **Evidence tier.** T1.
- **Answerability.** ANSWERABLE.
- **Depends on.** Q07 (TCT protocol), Q08 (threshold), Q09 (VALL-E 2 comparator).
- **Answer shape.** Protocol: one speaker (the author), 50-clip reference set, 10 held-out sentences, Qwen3-TTS inference-only + Full ICL + WavLM-scored best-of-12 at T=0.3. Results: WavLM SECS mean 0.961 (max 0.975) on the 10 unseen sentences; DNSMOS P.808 parity 3.93 vs 3.93; UTMOS parity 3.012 vs 2.997 vs the same speaker's real recordings. Scope scoping per FM-05 must be stated (single subject; $n=10$ held-out; author-is-subject).

### Q13 — What is projected (T3) but not yet measured for Case 1 (Shakespeare LoRA stack) and Case 2 (Santa identity-locked talking head)?
- **Section.** §6.1, §6.2 (headers tagged "T2+T3" per V20).
- **Evidence source.** Prior manuscript Case 1 / Case 2 sections; `scripts/shakespeare_full_pipeline.py`; Santa project `docs/inspect/08_training_audit.md`; mission brief.
- **Owner agents.** agent-38, agent-34 (confidence-quantifier), agent-43.
- **Evidence tier.** T2 (systems exist) + T3 (metrics pending).
- **Answerability.** PARTIAL — systems exist and run; metrics are pending. Flag per V22 with "(projected; camera-ready will re-verify)".
- **Depends on.** Q07, Q08.
- **Answer shape.** Case 1: 3-layer enforcement stack (SFT + DPO + runtime constellation guard) on cleaned Shakespeare corpus; 15/15 prior FSV passed (T2); SFT+DPO retrain in progress; projected metric: constellation-guard accept-rate on held-out First Folio lines — camera-ready will re-verify. Case 2: EchoMimicV3 + LoRA on 2,362 curated clips from the 16-min interview; mouth-weighted flow-matching loss; projected metric: visual-centroid cosine and lip-sync on unseen prompts — pending. Every number flagged.

### Q14 — What was Case 3's numerical delta vs. VALL-E 2's reported "human parity" and what is comparable vs. not comparable?
- **Section.** §6.3 (comparator paragraph); §8 Limitations.
- **Evidence source.** `_evidence_02_voice_cloning.md` (Chen et al., 2024 VALL-E 2); SITREP.
- **Owner agents.** agent-38, agent-36, agent-29.
- **Evidence tier.** T1 for the Case 3 numbers; T1 for the cited VALL-E 2 numbers; T4 for the "not directly comparable" structural argument.
- **Answerability.** ANSWERABLE with explicit comparability limits.
- **Depends on.** Q09, Q12.
- **Answer shape.** Case 3: WavLM SECS 0.961 on 10 held-out sentences, single speaker, cross-encoder WavLM (not used to condition the TTS). VALL-E 2: reports human parity (SIM, CMOS) on LibriSpeech-continuation with a single prompt per speaker and its own conditioning encoder. **Comparable**: both report speaker-similarity scores. **Not comparable**: different held-out sets, different encoders (V4 bare-SECS violation risk — always qualify the encoder), different prompt protocols (single-prompt vs. 50-clip centroid), different speaker cohorts (LibriSpeech vs. n=1 author). Comparability scoping is therefore narrow per FM-05.

---

## Category F — Sample complexity (Q15)

### Q15 — What is the sample-complexity collapse claim (single 16-min interview → identity-lock) and what is the evidence for it in each of the three case studies?
- **Section.** §3.6 or §5.4 (the claim); §6.1–§6.3 (per-case evidence).
- **Evidence source.** Mission brief core claim 4; SITREP; prior manuscript §6.
- **Owner agents.** agent-21, agent-38, agent-34.
- **Evidence tier.** T4 for the claim's structural form; T1 Case 3; T2+T3 Cases 1–2.
- **Answerability.** PARTIAL — claim is falsifiable only where measurement exists.
- **Depends on.** Q07, Q12, Q13.
- **Answer shape.** Restated precisely: with DDA-enriched data + TCT on top, identity-locked generation in three modalities requires "one reference set per modality + frozen constellation" rather than thousands of subject-specific examples. Evidence: Case 3 (one 50-clip reference set → 0.961 SECS; T1); Case 2 (one 2,362-clip reference set from a 16-min interview + ClipCannon decomposition → 12,000+ labeled signals; T2 system runs, T3 SECS/lip-sync pending); Case 1 (Shakespeare LoRA stack on cleaned corpus; projected guard accept-rate T3). The claim should be stated narrowly: "in the three protocols studied"; do not generalize (FM-05).

---

## Category G — Threats to validity (Q16–Q17)

### Q16 — What could invalidate the DDA / TCT claims (embedder non-independence, author-is-subject bias, cherry-picked held-out set, overfitting to the reference set, threshold calibration sensitivity)?
- **Section.** §8 Limitations.
- **Evidence source.** Glossary #14 (orthogonality hedge); FM-02, FM-05; R01.
- **Owner agents.** agent-29, agent-30 (risk-analyst), agent-42.
- **Evidence tier.** T4 threat model.
- **Answerability.** ANSWERABLE.
- **Depends on.** Q04, Q08, Q12, Q15.
- **Answer shape.** Enumerate at minimum: (a) embedder non-independence — DDA's $\binom{N}{2}$ pairwise signals degrade as embedder correlations rise; formal independence not measured (V11 hedge); (b) author-is-subject in Case 3 (FM-05); (c) 10-sentence held-out set is small; no confidence interval yet; (d) overfitting to the reference set — runtime guard uses the same embedders that built the centroid; discussed as "scoring-bias mitigation via cross-encoder" per glossary #20; (e) threshold $\tau_m$ is calibrated empirically on the subject's own recordings, not universally.

### Q17 — What forensic failures preceded the working method, and what do they teach a replicator?
- **Section.** §6 case-study "Forensics and corrections" paragraphs (per FM-08 counter-behavior); §8.4.
- **Evidence source.** Shakespeare training-data echo-bug FSV log; Santa `docs/inspect/08_training_audit.md` (flow-matching sign-bug); prior 17-attempt history; the cv2 frame-rate bug; the YOLOv5-Face mouth-landmark bug; glossary #29, #30.
- **Owner agents.** agent-38, agent-39 (discussion-writer), agent-42.
- **Evidence tier.** T2 (bug artifacts exist in forensic reports); T4 (structural lessons).
- **Answerability.** ANSWERABLE.
- **Depends on.** Q12, Q13.
- **Answer shape.** Compact per-bug paragraph: symptom → diagnostic method → root cause → fix → post-fix verification. (i) Shakespeare echo-bug — training data contained gold-response echoes; cleaning script; post-clean FSV 15/15. (ii) Santa flow-matching sign convention — target = noise − latent (glossary #30); flipped sign caused anti-denoising; forensic file cited. (iii) cv2 frame-rate assumption bug (prior 17-attempt history) — frame rate read from container vs. stream. (iv) YOLOv5-Face mouth-landmark bug — incorrect landmark index in mouth mask. Each bug cited to the specific file path per P10.

---

## Category H — Reproducibility, ethics, future work (Q18–Q20)

### Q18 — What does a replicator need to reproduce each case study (hyperparameters, data splits, frozen encoder versions, command line, evaluation script), and what is proprietary vs. substitutable?
- **Section.** §8.5 Reproducibility notes + Appendix.
- **Evidence source.** P08; prior manuscript reproducibility section; SITREP; Santa inspect docs.
- **Owner agents.** agent-43, agent-37, agent-25.
- **Evidence tier.** T2 (artefacts) + T4 (procedure).
- **Answerability.** ANSWERABLE for Case 3; PARTIAL for Cases 1–2 (subject-identifying material is proprietary / identity-sensitive; substitutable public approximation named).
- **Depends on.** Q07, Q10, Q11, Q12, Q13.
- **Answer shape.** For each case: (a) frozen encoder names + versions (WavLM-SV which checkpoint; Qwen3-TTS which version; CLIP/SigLIP versions for Case 2), (b) hyperparameters (T=0.3, best-of-12, $\tau_m$ values), (c) data split (10 held-out sentences, their source corpus), (d) evaluation script / command, (e) what is proprietary (subject recordings, Santa interview) + the substitutable public corpus a replicator can use (e.g., LibriSpeech subset; an alternate short interview). Every item maps to a reproducibility-checker row (agent-43).

### Q19 — What are the ethical considerations (identity-cloning consent, impersonation misuse, downstream voice-clone/video-clone attack vectors) and what mitigations or disclosures does the paper include?
- **Section.** §8.6 Ethics.
- **Evidence source.** Mission brief author-is-subject framing; prior manuscript ethics section; industry norms (voice-clone consent literature in `_evidence_02_voice_cloning.md`).
- **Owner agents.** agent-31 (ethics-reviewer), agent-39.
- **Evidence tier.** T4.
- **Answerability.** ANSWERABLE.
- **Depends on.** Q12, Q13, Q15.
- **Answer shape.** (a) Author-consent: subject = author for Case 2 and Case 3, so consent is explicit; note as scope limit (FM-05). (b) Misuse: identity-locked generation can be re-pointed to non-consenting subjects; discuss watermarking / authentication posture and the constellation guard's role as a *detector* as well as an enforcer (the guard can reject AND verify). (c) Downstream safety-profile constellations as future work (Q20). No exaggerated safety claim.

### Q20 — What future work extends this paper (formal embedder-independence measurement, hierarchical constellations, safety-profile constellations, cross-language / cross-demographic robustness, cross-encoder generalization)?
- **Section.** §9 Conclusion or §8.7 Future Work.
- **Evidence source.** Open questions flagged by glossary #14 (orthogonality hedge); FM-05 (scope); P07 (novelty delta).
- **Owner agents.** agent-40 (conclusion-writer), agent-18 (gap-hunter), agent-19 (opportunity-identifier).
- **Evidence tier.** T4.
- **Answerability.** ANSWERABLE.
- **Depends on.** Q16, Q18.
- **Answer shape.** At least five directions: (i) formally measure embedder independence (MI estimators across $\phi_j, \phi_k$) to replace V11's hedge; (ii) hierarchical constellations (subject → subject-in-context → subject-in-behavior) for longer-horizon generation; (iii) safety-profile constellations (safety centroid as an additional modality with its own $\tau$); (iv) cross-language / cross-demographic robustness on larger speaker cohorts; (v) cross-encoder generalization (guard scored by encoder $\neq$ training-loss encoder).

---

## Appendix — Question-to-claim coverage matrix

| Claim / system / study          | Questions                             |
|---------------------------------|---------------------------------------|
| Data-wall framing               | Q01, Q02, Q03                         |
| DDA definition + count          | Q04, Q16                              |
| Meaning-compression taxonomy    | Q05                                   |
| TCT method + runtime guard      | Q06, Q07, Q08, Q09                    |
| Context Graph (system, T2)      | Q10                                   |
| ClipCannon (system, T2)         | Q11                                   |
| Case 1 Shakespeare (T2+T3)      | Q13, Q17                              |
| Case 2 Santa (T2+T3)            | Q13, Q17                              |
| Case 3 Voice cloning (T1)       | Q12, Q14                              |
| Sample-complexity collapse      | Q15                                   |
| Threats to validity             | Q16                                   |
| Forensics / failure disclosure  | Q17                                   |
| Reproducibility                 | Q18                                   |
| Ethics                          | Q19                                   |
| Future work                     | Q20                                   |

Every core claim has ≥ 2 supporting questions. Every case study has ≥ 2 supporting
questions. No question is orphaned; each names an owner agent and an evidence source.

---

## Change log

- 2026-04-16 — Initial lock by agent-03-self-ask-decomposer.
