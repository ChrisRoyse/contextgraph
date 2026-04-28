---
namespace: patterns
agent_id: agent-13-quality-assessor
timestamp: 2026-04-16T23:40:00Z
status: active
locked: false
supersedes: none
tags: quality-summary, pool-stats, cluster-quality, systemic-concerns
depends_on:
  - memory/discoveries/agent-13-quality-assessor--per-citation-scores.md
  - memory/discoveries/agent-13-quality-assessor--quality-flags.md
downstream_consumers:
  - agent-14-bias-detector
  - agent-35-introduction-writer through agent-41-abstract-writer
  - agent-42-adversarial-reviewer
---

# Pool-Level Quality Summary

## Scored population

- **Hot tier (full):** 18 citations, every one scored on 5 dims × 0/1/2.
- **Warm tier (spot check):** 10 citations across clusters B, D, E, G, H.
- **Cold tier:** not individually scored — cold tier citations are framing /
  industry-signal references that cite with one-line role attribution. Cold
  tier is already low-traffic by design; per-citation scoring would waste
  writer attention.

## Headline statistics

| Statistic | Hot (18) | Warm spot (10) | Combined (28) |
|---|---|---|---|
| Mean score | 8.0 / 10 | 9.0 / 10 | 8.4 / 10 |
| Median | 9 | 9 | 9 |
| Fraction ≥ 8 (high quality) | 14 / 18 (77.8%) | 9 / 10 (90%) | 23 / 28 (82.1%) |
| Fraction ≥ 7 (acceptable) | 16 / 18 (88.9%) | 9 / 10 (90%) | 25 / 28 (89.3%) |
| Fraction < 7 (flagged) | 3 (16.7%) | 1 (10%) | 4 (14.3%) |
| Fraction < 5 (RED) | 2 (11.1%) | 0 (0%) | 2 (7.1%) |

## Per-cluster quality (hot + warm spot-checks combined)

| Cluster | N scored | Mean | Flagged | Notes |
|---|---|---|---|---|
| A Data wall | 3 | 9.0 | 0 | All three hot cites ≥ 8 (villalobos 9, hoffmann 8, longpre 10). |
| B Model collapse | 4 | 10.0 | 0 | **Cleanest cluster in the pool.** 4 cites, 4 × 10 / 10. Academic authors, open code, rigorous theory. |
| C Compression | 2 | 8.0 | 0 | deletang 9, huang 7 (SINGLE-SOURCE attribution required). |
| D Embedders | 4 | 9.0 | 0 | chen2022wavlm 9, zhai2023siglip 9, malkov2018hnsw 9, santhanam 9. |
| E TTS | 5 | 7.4 | 2 | **Most quality concerns.** valle2 4, naturalspeech3 5, maskgct 9, dnsmos 9, utmos 9. Closed industry-lab pattern. |
| F Video + flow-matching | 4 | 8.0 | 1 | echomimicv3 6, hu-lora 9, lipman 8, li-flame 9. |
| G Pipeline | 1 | 9.0 | 0 | wav2vec 9. |
| H Enterprise | 1 | 10.0 | 0 | pile-of-law 10. |
| I Alignment | 3 | 6.7 | 1 | **Second most quality concerns.** ouyang 7, rafailov 9, bai 4. The RLHF-lineage closed-artefact problem. |

## Which clusters have quality concerns?

**Two concentrated concerns, both traceable to a single systemic cause.**

### Concern 1 — Cluster E (TTS): 2 of 5 flagged (40%)

- **Root cause.** Microsoft + Microsoft-affiliated TTS labs (VALL-E 2,
  NaturalSpeech 3) systematically withhold weights, training data, and
  evaluation scripts while claiming SOTA numbers.
- **Impact on manuscript.** Case 3's flagship comparison (0.961 SECS vs.
  VALL-E 2 SIM-o) inherits the closed-comparator problem. Writers must hedge
  as specified in `agent-13-quality-assessor--quality-flags.md`.
- **Mitigation.** §6.3 uses open-source scorers (WavLM, DNSMOS, UTMOS) — our
  metrics are reproducible even if the comparators are not. §2.5 should
  preferentially pair closed comparators with open alternatives (MaskGCT,
  F5-TTS).

### Concern 2 — Cluster I (Alignment): 1 of 3 flagged (33%)

- **Root cause.** Industry-lab alignment papers (InstructGPT from OpenAI,
  Constitutional AI from Anthropic) also systematically withhold weights and
  data. DPO (Rafailov, Stanford) is the outlier — a reproducible closed-form
  derivation from academic authors.
- **Impact on manuscript.** §5 uses the RLHF / DPO / CAI triad as the
  scalar-reward comparator. Constitutional AI is the weakest of the three;
  writers must cite only as paradigm anchor.
- **Mitigation.** §2.6 + §5 explicitly name the contrast with the *paradigm*
  (scalar reward), not with specific reproducible artefacts. DPO is the
  well-supported anchor; CAI + InstructGPT function as positioning.

## Which clusters are clean?

**Cluster B (model collapse)** is the cleanest — 4 hot/warm citations, all
score 10 / 10. All academic authors, all open code, all rigorous theory,
all explicit limitations. This cluster is load-bearing for §2.2 + §3.3 + §8
and provides the strongest empirical grounding in the paper.

**Cluster A (data wall)** is very clean — all three hot cites ≥ 8.

**Cluster D (embedders)** is clean — all hot + warm cites ≥ 9. DDA substrate
is well-supported.

## Pool composition check

| Source-tier quality | Cross-checked with methodological-quality score |
|---|---|
| T1 papers (hot + warm spot) | 17 scored, mean **8.6** — high quality in both dimensions |
| T2 papers (hot + warm spot) | 10 scored, mean **7.7** — slightly lower, driven by cluster-E / I closed-artefact issues |
| T3 papers | 1 scored (ekman1978facs, warm) would likely score around 5-6 (reference work, not empirical) |

This two-axis view (source-tier × methodological-quality) reveals that the
T1/T2 cut is a **necessary but not sufficient** quality filter. A T1 paper can
still fail on reproducibility (VALL-E 2 would have been T1 at a venue;
NaturalSpeech 3 is T1 at ICML 2024 and still scored 5/10).

## Systemic finding — the closed-artefact problem

**Four flagged citations share a single root cause:** industry ML labs
(Microsoft for TTS, Anthropic for alignment, Alibaba for video) routinely
release papers claiming performance improvements without releasing weights,
code, or training data. The peer-review process (NeurIPS / ICML / arXiv) does
not currently enforce artefact release as a submission requirement.

**Paper-wide mitigation adopted.**
1. **Case 3 measurements use open scorers** (WavLM, DNSMOS, UTMOS) so our
   numbers are reproducible even when comparators' numbers are not.
2. **Case 2 forensic documents the sign-convention issue** EchoMimicV3's
   paper omits, turning a comparator weakness into a reproducibility
   contribution of our own.
3. **§5 contrasts TCT with the alignment *paradigm*** rather than with
   specific closed artefacts, so the argument does not depend on replicating
   CAI / InstructGPT / VALL-E 2 internals.
4. **§8 Limitations collects all closed-comparator caveats in one
   paragraph** rather than repeating them individually.

## Pool-level conclusion

- **82% of the scored pool (23/28) scores ≥ 8** on the rubric. This is high.
- **No hot-tier citation scored 0** — the minimum is 4 (VALL-E 2, Constitutional
  AI). These two are paradigm anchors, not evidence; the manuscript's use of
  them matches their role.
- **The three flagged hot-tier citations do not undermine any load-bearing
  claim** because each is cited in a role (comparator, base-model, paradigm
  anchor) that does not require it to be reproducible. The hedges in
  `quality-flags.md` make this explicit to readers.
- **The warm-tier 1 flag (NaturalSpeech 3)** is balanced by MaskGCT (open) and
  F5-TTS (open) in the same cluster; writers should pair them.

## What agent-14 (bias-detector) should look at next

1. **Affiliation clustering.** 3 of 4 flagged citations are from big-tech
   industry labs (Microsoft × 2, Anthropic × 1). Cluster E has a Microsoft
   balance issue; Cluster F has an Alibaba balance issue. Agent-11 already
   flagged this; agent-14 should quantify.
2. **Geographic distribution.** Pool is heavily US + China; European + academic
   author representation needs cross-check against the pool balance file.
3. **Citation recency bias.** 2023–2025 papers dominate — agent-14 to check
   whether older foundational work is under-cited (Kaplan 2020 is warm, not
   hot; Bengio 2003-style NLM foundational is absent).
4. **Gender / institutional diversity.** First-author patterns worth checking
   where disclosed.
5. **Venue bias.** Over-reliance on arXiv-only Microsoft / Alibaba preprints
   among flagged citations.

## Confidence

**HIGH.** Pool-level statistics are derived from explicit per-citation
scoring; cluster-level patterns trace to identifiable systemic causes
(industry-lab closed-artefact practice); no cluster-level conclusion is
dependent on a single citation's score.

## Changelog

- 2026-04-16 — Initial issue by agent-13-quality-assessor.
