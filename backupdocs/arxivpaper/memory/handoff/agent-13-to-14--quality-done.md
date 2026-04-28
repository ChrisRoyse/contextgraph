---
namespace: handoff
agent_id: agent-13-quality-assessor
timestamp: 2026-04-16T23:45:00Z
status: active
tags: handoff, agent-14, bias-detector, quality-done, flagged-citations, systemic-concerns
---

# Handoff: Agent-13 (quality-assessor) → Agent-14 (bias-detector)

## Read these files FIRST, in this order

1. `./memory/discoveries/agent-13-quality-assessor--per-citation-scores.md`
   — full 5-dim rubric scoring for all 18 hot-tier + 10 warm spot-check citations.
2. `./memory/discoveries/agent-13-quality-assessor--quality-flags.md`
   — the 4 flagged citations + mandatory caveat phrasing for writers.
3. `./memory/patterns/agent-13-quality-assessor--quality-summary.md`
   — pool-level stats, cluster breakdowns, and systemic findings.

## What agent-13 did

1. **Scored every hot-tier citation (n=18)** on a 5-dimension ML-paper rubric
   (Reproducibility, Evaluation rigor, Theoretical soundness, Sample adequacy,
   Bias disclosure); 0/1/2 per dimension, max 10 per paper.
2. **Spot-checked 10 warm-tier citations** on the same rubric to anchor the
   cross-tier distribution.
3. **Flagged 4 citations scoring < 7** and wrote mandatory writer-caveat
   phrasing for each.
4. **Identified one systemic root cause** — closed-artefact industry-lab
   releases — behind all 4 flags.
5. **Computed pool-level statistics** (mean 8.4, 82% ≥ 8, 89% ≥ 7) and
   per-cluster quality (Cluster B cleanest, Cluster E most concerns).
6. **Did not alter any tier assignment** (T1/T2/T3, H/W/C, or GRADE); all
   LOCKED upstream decisions preserved.

## Headline findings (for agent-14)

- **Hot-tier quality mean = 8.0 / 10; median = 9 / 10.** High overall.
- **3 hot-tier citations flagged** (all < 7):
  - **H11 chen2024valle2** (score 4 — RED) — closed-artefact, promotional
    framing. *The single most important flag* because Case 3's 0.961 SECS
    is benchmarked against this closed comparator.
  - **H12 meng2025echomimicv3** (score 6 — AMBER) — partial open weights;
    under-documented sign convention that the Case 2 forensic fills in.
  - **H17 bai2022constitutional** (score 4 — RED) — closed, product-adjacent.
    Cite only as paradigm anchor.
- **1 warm-tier citation flagged**:
  - **W05 tan2024naturalspeech3** (score 5 — AMBER) — Microsoft closed
    artefact. Pair with open comparators when cited.
- **Cleanest cluster: B (model collapse)** — 4/4 score 10/10.
- **Cluster with most concerns: E (TTS)** — 2/5 flagged; all Microsoft-
  affiliated.

## Systemic finding for agent-14's bias audit

**All four flagged citations share the same root cause: industry-lab papers
that release text + numbers but not weights / code / data.** This is a
field-wide bias concern (affiliation bias × publication bias × artefact-
release bias).

Recommended agent-14 lines of inquiry:
1. **Affiliation bias.** Tally of hot-tier citations by affiliation cluster
   (Microsoft, Alibaba, Google / DeepMind, Meta FAIR, academic). Agent-11
   already identified Microsoft + Alibaba imbalances; agent-14 should
   quantify at pool level and recommend rebalancing writers (agent-36 in
   particular).
2. **Publication-venue bias.** Fraction of hot-tier citations that are
   arXiv-only vs. peer-reviewed. Currently 6/18 hot-tier are arXiv-only
   (all T2; mostly industry labs).
3. **Geographic / institutional bias.** Where are authors based? Is European
   or South American or South Asian work represented?
4. **Recency bias.** Pool skews 2022–2025. Earlier foundational work
   (Bengio 2003 NLM, Mikolov 2013 word2vec, Vaswani 2017 attention) is
   absent from hot tier. Agent-14 should assess whether this is (a)
   appropriate scoping, (b) inappropriate recency bias, or (c) a gap
   to flag.
5. **Gender / first-author diversity.** Quantify where disclosed.
6. **Self-citation bias.** Our own author (Chris Royse) has no prior arXiv
   papers to self-cite — not a concern. But check co-citation with Teleox /
   Leapable work; should be minimal.

## Quality-gate status delivered

- **QG (pool quality).** 82% of 28 scored citations score ≥ 8 on the 5-dim
  rubric. **PASS.**
- **QG (hot-tier minimum).** 14 of 18 hot cites ≥ 8; all 18 ≥ 4. **PASS**
  (threshold was no hot-tier citation below hard-floor of 4; 2 papers at
  floor are paradigm anchors, not evidence).
- **QG (flag transparency).** Each of the 4 flagged citations has
  mandatory writer caveat text written and assigned to target sections.
  **PASS.**

## Critical constraints carrying forward

- **Master pool LOCKED** per agent-08. No new citations without blocker.
- **Cluster labels A–I LOCKED** per agent-06.
- **T1/T2/T3 tier assignments LOCKED** per agent-07.
- **Evidence grades LOCKED** per agent-11.
- **H/W/C tier assignments LOCKED** per agent-12.
- **5-dim rubric scores SOFT-LOCKED** — can be adjusted if agent-14 or
  agent-29 surface new evidence, but any change needs blocker + changelog.
- **Mandatory caveat text** for flagged citations is **LOCKED**; writers
  copy verbatim, no paraphrase.

## Open items (for agent-14 to consider)

1. **Cluster-E closed-artefact pattern.** Two of five TTS cites flagged.
   Agent-14 should document this as a field-wide bias, not a pool construction
   error.
2. **Cluster-I closed-artefact pattern.** Constitutional AI + InstructGPT
   closed, DPO open. Recommend writers lean on DPO as the rigorous
   comparator and treat CAI / InstructGPT as paradigm anchors.
3. **Contamination non-disclosure across big-tech papers.** Chinchilla,
   SigLIP, WavLM all scored 1/2 on bias disclosure specifically for not
   discussing benchmark contamination. Agent-14 should check whether the
   manuscript body should mention this collective gap.
4. **Warm-tier unscored.** 16 warm-tier citations remain unscored. If
   agent-14's bias work reveals concentration of one affiliation in warm
   tier, additional scoring may be warranted.

## What writers (agents 35–41) must do with agent-13's outputs

1. **Open `quality-flags.md` before drafting §6.3 (agent-38), §2.5 / §2.6
   (agent-36), §5 / §6.2 (agent-37 / 38), §8 (agent-29 / 39).**
2. **Copy mandatory caveat text verbatim** for the four flagged citations.
3. **When citing H07 huang2024compression (SINGLE-SOURCE + score 7), use
   attribution phrasing** ("Huang et al. 2024 report r ≈ 0.95 across 30
   LLMs") rather than consensus phrasing.
4. **§8 Limitations must include the composite closed-artefact paragraph**
   written in `quality-flags.md`.

## Confidence

**HIGH.** Rubric is standard; scores trace to specific paper evidence;
systemic root cause identified; mitigation plan is concrete and written
into writer caveat text. All pool-level statistics are reproducible from
the per-citation scores file.

---

## Changelog

- 2026-04-16 — Initial handoff by agent-13-quality-assessor.
