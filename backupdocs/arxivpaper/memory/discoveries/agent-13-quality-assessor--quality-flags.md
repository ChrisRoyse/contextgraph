---
namespace: discoveries
agent_id: agent-13-quality-assessor
timestamp: 2026-04-16T23:35:00Z
status: active
locked: false
supersedes: none
tags: quality-flags, caveats, low-quality-citations, writer-cautions
depends_on:
  - memory/discoveries/agent-13-quality-assessor--per-citation-scores.md
downstream_consumers:
  - agent-14-bias-detector
  - agent-29-validity-guardian
  - agent-36-related-work-writer
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-39-discussion-writer
  - agent-42-adversarial-reviewer
  - agent-44-apa-citation-specialist
---

# Quality Flags — Citations Requiring Writer Caveats

Four citations scored **< 7** on the five-dimension rubric (three in hot tier,
one in warm spot-checks). Each requires explicit caveat phrasing when cited in
the manuscript body.

## Flag severity scheme

- **RED (< 5).** Cite only as paradigm-positioning, never as empirical
  evidence. Always attribute (author-year-report phrasing). Always hedge
  reproducibility.
- **AMBER (5 ≤ score < 7).** Cite with attribution. Disclose weights / data /
  code status. Use V12 / V17 / V22 hedge where claim is AUTHOR-EMPIRICAL.

---

## FLAG 1 — H11 chen2024valle2 (Score 4 / 10) — RED

**Citation.** Chen, S., Liu, S., Zhou, L., et al. (2024). VALL-E 2: Neural
codec language models are human parity zero-shot text-to-speech synthesizers.
arXiv:2406.05370.

**Why flagged.**
- **Reproducibility 0/2.** No weights, no code, no training data released.
  Microsoft-internal.
- **Bias disclosure 0/2.** "Human parity" framing is promotional; no
  contamination discussion; no safety discussion.
- Theoretical soundness 1/2 — "human parity" claim is supported by CMOS
  ≈ 0 but the comparison pool is small; scope creep risk.

**Hot-tier role.** Primary comparator for Case 3's 0.961 SECS claim (CL-15,
CL-16). This is **the** external benchmark the paper's strongest empirical
result is positioned against.

**Mandatory writer caveat when citing in §6.3 (Results — Case 3).**

> "Chen et al. (2024) report a SIM-o of approximately 0.64 on their internal
> LibriSpeech evaluation split (weights, data, and evaluation scripts were not
> released). Our 0.961 WavLM SECS is measured on a publicly reproducible
> scorer (Chen et al., 2022 WavLM-large) over 10 unseen sentences from a
> single speaker (Case 3); the two numbers are not directly comparable
> without accounting for (i) different speaker embedding extractors, (ii)
> different test-sentence pools, and (iii) different cohort compositions.
> We follow the methodological practice of reporting *our* metric transparently
> rather than claiming direct SIM-o parity."

**Mandatory writer caveat when citing in §2.5 (Related Work — TTS).**

> "VALL-E 2 claims 'human parity' on internal LibriSpeech metrics. The
> artefact is closed (no weights, no data, no code), so community
> verification is not currently possible."

**Mandatory writer caveat when citing in §8 (Limitations).**

> "Case 3's comparison to VALL-E 2 inherits the closed-artefact limitation of
> the comparator; we mitigate by reporting on an open-source scorer (WavLM)
> and releasing evaluation audio, but cannot reproduce VALL-E 2's own eval
> protocol."

---

## FLAG 2 — H12 meng2025echomimicv3 (Score 6 / 10) — AMBER

**Citation.** Meng, R., Wang, Y., Wu, W., et al. (2025). EchoMimicV3: 1.3B
parameters are all you need for unified multi-modal and multi-task human
animation. arXiv:2507.03905.

**Why flagged.**
- **Reproducibility 1/2.** Open-weight release is a plus; training code partial;
  training data undisclosed.
- **Theoretical soundness 1/2.** Flow-matching sign-convention is
  under-documented in the paper — the Case 2 Santa forensic *is* the disclosure
  the paper lacks. See CL-21.
- **Sample adequacy 1/2.** Training corpus undisclosed.
- **Bias disclosure 1/2.** Alibaba authors; no safety / deepfake ethics section.

**Hot-tier role.** Base model for Case 2 (Santa) + anchor for flow-matching
sign-convention forensic. Four Q-IDs (Q07, Q11, Q13, Q17).

**Mandatory writer caveat when citing in §6.2 (Results — Case 2 Santa).**

> "Case 2 builds on EchoMimicV3 (Meng et al., 2025). We note that the base
> paper does not fully document the flow-matching sign convention used in
> training; our LoRA fine-tuning uncovered a sign mismatch between community
> implementations that required a forensic diagnosis documented in
> docs/inspect/08_training_audit.md of the Santa project. Readers applying
> LoRA on top of EchoMimicV3 should verify sign convention against the
> base-model release."

**Mandatory writer caveat when citing in §2.5 (Related Work — video).**

> "EchoMimicV3 released open inference weights (partial training recipe;
> training data undisclosed)."

---

## FLAG 3 — H17 bai2022constitutional (Score 4 / 10) — RED

**Citation.** Bai, Y., Kadavath, S., Kundu, S., et al. (2022). Constitutional
AI: Harmlessness from AI feedback. arXiv:2212.08073.

**Why flagged.**
- **Reproducibility 0/2.** No weights, no code, no RLAIF data.
- **Evaluation rigor 1/2.** Elo / win-rate evals only; no public benchmarks
  compared with statistical rigor.
- **Theoretical soundness 1/2.** "Constitution" is a list of heuristics; the
  RLAIF scalability claim is speculative.
- **Sample adequacy 1/2.** One model size (Claude-adjacent ~52B).
- **Bias disclosure 1/2.** Anthropic conflict; product-adjacent framing.

**Hot-tier role.** Third anchor of the RLHF / DPO / CAI scalar-reward triad
(CL-14, CL-28) — used to establish the *paradigm* TCT contrasts against.

**Mandatory writer caveat when citing in §2.6 (Related Work — alignment).**

> "Constitutional AI (Bai et al., 2022) proposed aligning language models via
> AI-generated feedback constrained by a written constitution. The work is a
> closed-artefact technical report; the 'constitution' is a list of principles
> rather than a theoretical framework. We cite it as a canonical anchor of
> the *scalar-reward + AI-feedback* alignment paradigm, not as a reproducible
> empirical result."

**Mandatory writer caveat when citing in §5 (TCT method).**

> "The scalar-reward alignment paradigm — here represented by RLHF (Ouyang
> et al., 2022), DPO (Rafailov et al., 2023), and Constitutional AI (Bai et
> al., 2022) — reduces alignment to a scalar objective. TCT operates outside
> this paradigm: cosine proximity to a frozen multi-modal centroid is
> per-output verifiable and does not require a reward model."

**Do NOT cite Bai 2022 as evidence for any empirical claim** — cite only as
a scope / paradigm anchor.

---

## FLAG 4 — W05 tan2024naturalspeech3 (Score 5 / 10) — AMBER

**Citation.** Tan, X., Ju, Z., Wang, Y., et al. (2024). NaturalSpeech 3:
Zero-shot speech synthesis with factorized codec and diffusion models.
*ICML 2024*. arXiv:2403.03100.

**Why flagged (warm-tier; not hot).**
- **Reproducibility 0/2.** Microsoft; no weights, no code, no training data.
- **Bias disclosure 0/2.** "Natural speech" framing promotional.
- Evaluation rigor 2/2; theoretical soundness 2/2; sample adequacy 1/2 (not
  enough to lift total above 5).

**Warm-tier role.** Secondary TTS comparator for §2.5 / §6.3 context (CL-15).

**Recommended writer caveat when citing in §2.5.**

> "NaturalSpeech 3 (Tan et al., 2024) reports state-of-the-art SIM-o and WER
> on LibriSpeech using a factorized codec + diffusion pipeline. Like VALL-E 2,
> the artefact is closed; benchmark numbers are not community-verifiable."

**Recommended writer practice.** When §2.5 enumerates TTS comparators,
**prefer citing wang2024maskgct (Amphion, open) alongside NaturalSpeech 3** —
the reader should see at least one open-source comparator in the same paragraph.

---

## Composite caveat for §8 Limitations (all four flags)

Writer of §8 Limitations should include a paragraph roughly:

> "Our related-work and comparison sections cite several closed-artefact
> industry releases (VALL-E 2, NaturalSpeech 3, Constitutional AI,
> EchoMimicV3 partial). These papers report strong results but do not
> release weights, training code, or training data. Our manuscript reports
> measurements using fully open-source scorers (WavLM, DNSMOS, UTMOS) on a
> fully specified case-study pipeline; we acknowledge that direct numerical
> comparison with closed-artefact baselines is inherently limited, and we
> recommend the reader treat the closed comparators as *paradigm anchors*
> rather than as reproducible benchmarks."

---

## Citations NOT flagged but requiring attribution-phrasing reminder

These scored ≥ 7 but still carry SINGLE-SOURCE status per agent-11 GRADE
table. Writers must attribute explicitly (per agent-12 SINGLE-SOURCE
attribution rule) but no quality-flag caveat is needed beyond that.

| Key | Score | Why attribution required |
|---|---|---|
| H07 huang2024compression | 7 | SINGLE-SOURCE for r ≈ 0.95 claim |
| H13 hu2022lora | 9 | Foundational single-source |
| H14 lipman2023flowmatching | 8 | Foundational single-source |
| H10 malkov2018hnsw | 9 | Foundational single-source |
| H03 longpre2024consent | 10 | SINGLE-SOURCE for accessible-fraction |
| W10 henderson2022pileoflaw | 10 | SINGLE-SOURCE for Pile of Law |

## OpenAI / industry-lab papers requiring disclosure (per-paper)

| Key | Reproducibility score | Writer note |
|---|---|---|
| H15 ouyang2022instructgpt | 0/2 | Closed weights; cite as paradigm anchor (OK within the scalar-reward triad). |
| H11 chen2024valle2 | 0/2 | **FLAGGED** — see Flag 1. |
| H17 bai2022constitutional | 0/2 | **FLAGGED** — see Flag 3. |
| W05 tan2024naturalspeech3 | 0/2 | **FLAGGED** — see Flag 4. |

## Confidence

**HIGH.** Each flag is specific, has a root cause, and has concrete caveat
text for the writer to copy in the relevant section. The four flagged
citations represent a single systemic pattern (closed-artefact industry
releases) which §8 Limitations should address once collectively rather than
repeating individually.

## Changelog

- 2026-04-16 — Initial issue by agent-13-quality-assessor.
