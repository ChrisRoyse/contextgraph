---
namespace: patterns
agent_id: agent-14-bias-detector
timestamp: 2026-04-16T23:59:00Z
status: active
locked: false
supersedes: none
tags: bias-mitigations, writer-snippets, copy-verbatim, limitations-text, caveat-phrasing
depends_on:
  - memory/discoveries/agent-14-bias-detector--bias-audit.md
  - memory/discoveries/agent-13-quality-assessor--quality-flags.md
downstream_consumers:
  - agent-15-evidence-synthesizer
  - agent-35-abstract-writer
  - agent-36-related-work-writer
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-39-discussion-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
---

# Bias-Mitigation Snippets — Copy-Verbatim Prose for Writers

This file is the **writer-facing companion** to the bias audit
(`discoveries/agent-14-bias-detector--bias-audit.md`). Each snippet is
designated for a specific target section, keyed to the bias it mitigates,
and **must be copied verbatim** unless agent-42 (adversarial-reviewer)
asks for a rewrite.

Three snippets are required in §8 Limitations (Bias-03, Bias-05, Bias-08);
the rest are single-line caveats inserted alongside the relevant claim or
citation.

Where a snippet overlaps with text already mandated by agent-13
(quality-flags), agent-13's phrasing takes precedence — the snippets
below are the *bias-layer* additions, not the citation-quality-layer
replacements.

---

## §1 Introduction — temporal framing caveat (Bias-10)

**Insert after the data-wall framing paragraph, before the CL-02
villalobos citation.**

> "In late 2024, multiple industry observers reported diminishing
> returns from pre-training-scale scaling (Franzen 2024; Metz 2024;
> Hu 2024; Woo 2024; Sutskever 2024). These reports cross-reference
> each other and are best read as one narrative signal from that
> period rather than as five independent confirmations. The peer-
> reviewed literature on data-stock exhaustion (Villalobos et al.
> 2024; Longpre et al. 2024) provides the evidential base for the
> framing used in this manuscript."

---

## §2.5 Related Work — TTS — publication + affiliation + benchmark + temporal bias (Biases 01, 03, 09, 10)

**Insert at the end of the TTS subsection, before transitioning to
identity-locked video.**

> "This subsection cites representative zero-shot TTS systems that
> share a common evaluation vocabulary (SIM-o, WER, DNSMOS, UTMOS) and
> anchor the numerical comparator space for §6.3. It is not a
> systematic review of the field. Four of the frontier citations
> (Wang 2023; Chen 2024; Tan 2024; Shen 2023) are Microsoft-affiliated
> closed-artefact releases; we pair them with open-source comparators
> (Wang 2024 MaskGCT; Chen 2024 F5-TTS; Casanova 2024 XTTS) to reduce
> affiliation imbalance. TTS benchmarks in this literature are
> dominated by LibriSpeech- and VoxCeleb-derivative corpora, which are
> clean-audio convenience datasets with limited dialect, accent, and
> recording-condition diversity; comparator numbers inherit these
> scope boundaries. Non-English TTS literature (including substantial
> work on Mandarin, Japanese, and Korean synthesis) is out of scope of
> this review."

---

## §2.5 Related Work — identity-locked video — affiliation + venue bias (Bias-03)

**Insert at the end of the video subsection.**

> "Four of the identity-locked video citations (Meng 2025 EchoMimicV3;
> Tian 2024 EMO; Tian 2025 EMO2; Wan 2025 Wan-Video) are Alibaba-affiliated;
> eight of twelve are arXiv-only (not peer-reviewed at the time of
> this writing). We balance the cluster with Cui et al. 2024 Hallo3
> (Fudan-Baidu), Zheng et al. 2025 HunyuanVideo (Tencent), Yang et al.
> 2024 CogVideoX (Tsinghua-Zhipu), and Li et al. 2024 LatentSync
> (ByteDance). Substantive TTS and video frontier papers predominantly
> report positive results on internal benchmarks; we do not have
> access to independently replicated failure analyses and treat
> cluster-typical numbers as upper-bound frontier anchors rather than
> verified cross-lab comparisons."

---

## §2.6 Related Work — Alignment — citation selection bias (Bias-02)

**Insert immediately before the RLHF / DPO / CAI triad is introduced.**

> "We cite three canonical scalar-reward alignment anchors (RLHF:
> Ouyang et al. 2022; DPO: Rafailov et al. 2023; Constitutional AI:
> Bai et al. 2022) because the TCT contrast in §5 is against their
> shared scalar-reward / preference-model primitive. We do not claim
> to survey the broader alignment landscape (e.g., RLAIF, KTO, IPO,
> ORPO); the 3-paper, US-only scope is a deliberate contrast device
> and is re-noted in §8."

---

## §6.1 Results — Case 1 Shakespeare — confirmation-bias framing (Bias-07)

**Use as the opener of §6.1 after the one-line Case 1 scope statement.**

> "The Case 1 narrative below describes the engineering history of
> the Shakespeare LoRA stack: the training-data echo-bug discovered
> during development, its diagnostic method, and the corrective
> fix that preceded the reported 15/15 FSV passes. The forensic is
> presented as engineering evidence; it is not a randomised ablation
> study, and alternative corrections may exist."

---

## §6.2 Results — Case 2 Santa — single-subject, forensic-framing, publication bias (Biases 05, 07, 01)

**Use as the opener of §6.2 — replaces any other Case 2 scope sentence.**

> "Case 2 trains on 2,362 curated clips from a single 16-minute
> interview of one subject (n = 1). The case establishes that the
> EchoMimicV3 + LoRA pipeline runs end-to-end at this data scale and
> that the flow-matching sign-convention issue can be forensically
> diagnosed and fixed; it does not claim generalisation to other
> subjects. The base model (Meng et al. 2025 EchoMimicV3) does not
> fully document the flow-matching sign convention used in training;
> our LoRA fine-tuning uncovered a sign mismatch that required a
> forensic diagnosis documented in
> `docs/inspect/08_training_audit.md` of the Santa project. Readers
> applying LoRA on top of EchoMimicV3 should verify sign convention
> against the base-model release."

---

## §6.3 Results — Case 3 voice cloning — single-subject, single-encoder, self-reporting, benchmark, English-only (Biases 05, 06, 08, 09, 04)

**Use as the opener of §6.3 — must contain the full scope statement.**

> "Case 3 evaluates a single speaker (the author) reading 10 held-out
> English sentences. The reference corpus is a 50-clip L2-normalised
> centroid from the same speaker; generations are WavLM-SECS-scored
> best-of-12 at T=0.3; DNSMOS P.808 and UTMOS naturalness scores are
> computed against the same speaker's real recordings. We do not
> generalise beyond this protocol (no cross-speaker, no cross-language,
> no cross-dialect, no cross-recording-condition evaluation). SECS is
> computed on a single speaker encoder (WavLM-large, Chen et al. 2022);
> cross-encoder audit (ECAPA-TDNN / Resemblyzer) is left to a
> follow-up. The reported numbers are author-measured on author-built
> pipelines using open-source scorers and have not been independently
> replicated at submission time."

---

## §6.3 — VALL-E 2 comparator caveat (Biases 01, 03)

**This is agent-13's FLAG-1 caveat, re-stated here for cross-reference.
Copy verbatim when citing Chen et al. 2024 (VALL-E 2) in §6.3.**

> "Chen et al. (2024) report a SIM-o of approximately 0.64 on their
> internal LibriSpeech evaluation split (weights, data, and
> evaluation scripts were not released). Our 0.961 WavLM SECS is
> measured on a publicly reproducible scorer (Chen et al., 2022
> WavLM-large) over 10 unseen sentences from a single speaker
> (Case 3); the two numbers are not directly comparable without
> accounting for (i) different speaker embedding extractors,
> (ii) different test-sentence pools, and (iii) different cohort
> compositions. We follow the methodological practice of reporting
> *our* metric transparently rather than claiming direct SIM-o
> parity."

---

## §7 Discussion — encoder-scope and cross-speaker hedge (Biases 05, 06)

**Insert wherever §7 discusses the 0.95 SECS barrier or the Case 3
result.**

> "The 0.95 WavLM-SECS threshold referenced in §6.3 is encoder-
> specific; the 0.961 / 0.975 figures we report reflect WavLM-large
> distance distribution on a single speaker's held-out sentences, not
> an absolute speaker-identity guarantee across encoders or across
> speakers. The direction of the result — exceeding a published
> similarity figure under a specified protocol — is the claim; the
> transportability of that result to other encoders or speakers is
> not."

---

## §7 Discussion — comparability to closed-artefact baselines (Bias-03)

**Insert where §7 compares our numbers to the cited frontier.**

> "Comparisons between our open-scorer single-speaker numbers
> (§6.3) and closed-artefact industry baselines (VALL-E 2,
> NaturalSpeech 3) are inherently limited: the baselines do not
> release weights, data, or evaluation code, and the reported
> metrics use different encoders and different test pools. We
> report our numbers on fully specified infrastructure and release
> held-out audio; we treat the cited frontier as a paradigm anchor
> rather than a directly comparable benchmark."

---

## §8 Limitations — REQUIRED PARAGRAPH 1 — industry-lab / affiliation bias (Bias-03)

**Copy verbatim.**

> "Our citation pool shows lab-concentration bias in three clusters.
> The zero-shot TTS cluster is Microsoft-heavy (Wang 2023; Chen 2024
> VALL-E 2; Tan 2024 NaturalSpeech 3; Shen 2023 NaturalSpeech 2 —
> four Microsoft citations). The identity-locked video cluster is
> Alibaba-heavy (Meng 2025 EchoMimicV3; Tian 2024 EMO; Tian 2025
> EMO2; Wan 2025 Wan-Video — four Alibaba citations) and arXiv-only
> for eight of twelve entries. The alignment cluster is US-only
> across three citations (OpenAI, Stanford, Anthropic). We mitigate
> by pairing closed-artefact industry citations with open-source
> comparators where possible (Wang 2024 MaskGCT, Chen 2024 F5-TTS,
> Casanova 2024 XTTS in TTS; Cui 2024 Hallo3, Zheng 2025 HunyuanVideo
> in video) and by reporting our Case 3 numbers on a fully open
> scorer (WavLM) over releasable audio. We do not claim the cited
> frontier captures the full field, only the most-referenced systems
> at the time of writing. Closed-artefact industry papers also
> score 0/2 on reproducibility in our quality rubric (see §8 supplementary
> table if included); we treat them as paradigm anchors rather than
> reproducible benchmarks."

---

## §8 Limitations — REQUIRED PARAGRAPH 2 — single-subject bias (Bias-05)

**Copy verbatim.**

> "Case 2 and Case 3 are n = 1 subject studies. Case 2's training
> corpus is drawn from a single 16-minute interview of one subject;
> Case 3's reference set and held-out sentences share a single
> speaker (the author). Single-subject evaluations are a structural
> feature of our current artefacts and are not a substitute for
> multi-speaker evaluation; we report numbers with this scope
> bounded and do not advance population-level claims. Case 1
> (Shakespeare) is a text-style study and does not use the single-
> subject frame."

---

## §8 Limitations — REQUIRED PARAGRAPH 3 — self-reporting bias (Bias-08)

**Copy verbatim.**

> "All numerical results in §6 (Case 1 FSV pass rates; Case 2
> forensic findings; Case 3 SECS / DNSMOS / UTMOS figures) are
> author-measured on author-built pipelines and have not been
> independently replicated at the time of submission. We mitigate
> the self-reporting concern by (i) using open-source scorers
> where possible (WavLM, DNSMOS, UTMOS), (ii) specifying the
> evaluation protocol in full so the measurement is reproducible,
> and (iii) releasing held-out evaluation audio. We acknowledge
> that third-party replication is the next step and is not yet in
> hand; we name it as follow-up work in §9."

---

## §8 Limitations — language, benchmark, encoder, temporal (Biases 04, 06, 09, 10)

**Insert as four short bullets or one compact paragraph, after the
three required paragraphs above.**

> "Additional scope boundaries: (i) the 88-citation pool is English-
> language only, and Case 3's evaluation uses English-only
> sentences by a single English-speaking subject — no tonal-language
> or cross-accent claims are made; (ii) Case 3 SECS is reported on a
> single speaker encoder (WavLM-large); cross-encoder audit is
> follow-up work; (iii) our evaluation corpus (10 held-out clean-
> audio sentences) is not a population-level speaker-diversity test
> and does not cover accents, dialects, or noisy recording
> conditions; (iv) clusters E, F, and the Cluster A T3 sub-set are
> temporally compressed around 2023–2025 / Q4-2024 press, and the
> 'frontier' and 'data-wall' framings reflect the discourse at the
> time of writing."

---

## §8 Limitations — citation selection scope (Bias-02)

**Insert immediately after the composite closed-artefact paragraph
(agent-13) or as the final Limitations paragraph.**

> "Our citation pool is thesis-aligned by design: clusters are
> selected to make DDA, TCT, and meaning-compression contrasts
> legible. The model-collapse literature (Cluster B) is included as
> explicit tension against DDA; other tensioning literatures —
> reward-modeling extensions beyond RLHF / DPO / Constitutional AI,
> non-English TTS benchmarks, video-generation failure-mode audits —
> are out of scope of this position manuscript."

---

## §9 Conclusion / Future Work — replication + cross-encoder (Biases 06, 08)

**Insert one future-work line.**

> "Third-party replication of Case 3's SECS, DNSMOS, and UTMOS
> numbers, and cross-encoder audit of the 0.961 figure on
> encoders beyond WavLM-large (e.g., ECAPA-TDNN, Resemblyzer),
> are named follow-up tasks."

---

## Abstract (§0) — scope discipline reminder (Biases 05, 08)

**Not a snippet to copy; a constraint on the abstract writer.**

- Do NOT write "we demonstrate voice cloning beyond 0.95 SECS".
- DO write "we report 0.961 mean WavLM-SECS on a single-speaker
  single-encoder 10-sentence evaluation (Case 3)".
- Do NOT write "across three case studies". DO write "on three case
  studies with distinct scope boundaries specified in §6".
- The 0.961 / 0.975 numbers are allowed in the abstract; the
  phrasing must preserve encoder + speaker + sentence-count scope.

---

## Agent-42 adversarial regex / read-through targets

Flag these phrases as bias-creep violations when detected in drafts:

- "systematically eliminated" / "principled derivation" in §6 → Bias-07
  retcon.
- "industry observers agree" / "consensus view" when T3 press citations
  follow → Bias-10 temporal inflation.
- "we demonstrate" when AUTHOR-EMPIRICAL hedge required → Bias-08.
- "across all speakers" / "generalises to" → Bias-05.
- "the TTS frontier achieves" without encoder / data scope → Bias-09.
- "revolutionary" / "groundbreaking" / any superlative → FM-01 marketing
  creep (agent-01); also inflates all biases simultaneously.

---

## Writer checklist — before submitting §2 / §6 / §7 / §8 drafts

- [ ] §1: Cluster A T3 signals framed as narrative moment (Bias-10).
- [ ] §2.5 TTS: Microsoft citations paired with open comparators
      (Bias-03, Bias-01).
- [ ] §2.5 video: Alibaba citations paired with Tencent / Fudan / ByteDance
      (Bias-03, Bias-01).
- [ ] §2.6 alignment: 3-paper scope disclosed (Bias-02).
- [ ] §6.1: forensic framed as engineering narrative, not systematic
      elimination (Bias-07).
- [ ] §6.2 opener: n = 1 subject scope stated (Bias-05).
- [ ] §6.3 opener: speaker + sentence + encoder + language scope stated
      (Biases 04, 05, 06, 08, 09).
- [ ] §6.3 VALL-E 2 caveat present when cited (Biases 01, 03).
- [ ] §7: no cross-speaker / cross-encoder claims (Biases 05, 06).
- [ ] §8: three required paragraphs (Biases 03, 05, 08) + compact
      boundary paragraph (Biases 04, 06, 09, 10) + selection scope
      (Bias-02).
- [ ] §9: third-party replication + cross-encoder named as follow-up
      (Biases 06, 08).

## Confidence

**HIGH.** All snippet text is grounded in the bias audit's quantified
findings and the LOCKED upstream quality-flag caveats from agent-13.
Snippets are section-keyed and ready for writer copy-verbatim use.

## Changelog

- 2026-04-16 — Initial issue by agent-14-bias-detector.
