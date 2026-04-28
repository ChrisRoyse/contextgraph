---
namespace: handoff
agent_id: agent-14-bias-detector
timestamp: 2026-04-17T00:05:00Z
status: active
tags: handoff, agent-15, evidence-synthesizer, biases-documented, mitigations-locked
---

# Handoff: Agent-14 (bias-detector) → Agent-15 (evidence-synthesizer)

## Read these files FIRST, in this order

1. `./memory/discoveries/agent-14-bias-detector--bias-audit.md` — the
   10-axis bias audit (full evidence per axis, severity, scope, and
   mitigation site).
2. `./memory/patterns/agent-14-bias-detector--bias-mitigations.md` —
   copy-verbatim snippets keyed to §1 / §2.5 / §2.6 / §6.1 / §6.2 /
   §6.3 / §7 / §8 / §9 / abstract.
3. `./memory/discoveries/agent-13-quality-assessor--quality-flags.md`
   — the four pre-existing citation-quality caveats whose phrasing
   dovetails with the bias snippets above.
4. `./memory/discoveries/agent-11-systematic-reviewer--cluster-biases.md`
   — the upstream cluster-level bias audit that agent-14 quantified
   and collapsed into paper-facing mitigation text.

## What agent-14 did

1. Audited ten bias axes specified in the task brief across (a) the
   88-citation pool and (b) the paper's own empirical claims.
2. Classified each as CRITICAL / HIGH / MODERATE / LOW with
   **structural vs. remediable** distinction.
3. Wrote section-keyed copy-verbatim mitigation snippets for writers
   (§1, §2.5 TTS, §2.5 video, §2.6 alignment, §6.1, §6.2 opener,
   §6.3 opener, §6.3 VALL-E 2 caveat, §7, §8 three required
   paragraphs, §9 follow-up, abstract discipline).
4. Delivered an agent-42 adversarial regex target list.
5. Delivered a writer-checklist covering §1 through §9.

## Headline findings for agent-15 (evidence synthesis)

### The three CRITICAL + STRUCTURAL biases (require §8 dedicated paragraphs)

1. **Bias-03 — Industry-lab / affiliation bias.** Microsoft × 4 in
   Cluster E (TTS), Alibaba × 4 in Cluster F (video), US-only × 3 in
   Cluster I (alignment). Correlated with closed-artefact bias
   (all 4 agent-13 flags originate here).
2. **Bias-05 — Single-subject bias (Case 2 n=1 interview; Case 3
   author-as-subject).** Cannot be fixed within this submission;
   must be scoped explicitly in §6.2, §6.3, §8.
3. **Bias-08 — Self-reporting bias (all case numbers author-measured).**
   Third-party replication is follow-up work; mitigated by open
   scorers + full protocol + releasable audio.

### The three biases that will need explicit Limitations text

Same three as above (Bias-03, Bias-05, Bias-08). The mitigation file
contains three copy-verbatim §8 paragraphs for these, plus a compact
fourth paragraph covering Biases 04 (English), 06 (encoder), 09
(benchmark), and 10 (temporal).

### The three biases that are structural features of our empirical claims

1. **Bias-05 single-subject** (Cases 2, 3) — n = 1 by design.
2. **Bias-06 single-encoder** (Case 3 WavLM-only) — encoder-specific
   similarity scale.
3. **Bias-08 self-reporting** (all cases) — first-party measurement
   inherent to novel-system papers.

### Biases that are remediable by writing discipline

- **Bias-01** (publication / positive-results frontier) — §2.5 caveat.
- **Bias-02** (citation selection scope) — §2.5 / §2.6 / §8 scope
  statements.
- **Bias-04** (English-only) — §8 bullet.
- **Bias-07** (confirmation / forensic retcon) — §6 framing discipline
  + agent-42 regex.
- **Bias-09** (benchmark diversity) — §2.5 + §6.3 scope statements.
- **Bias-10** (temporal 2024Q4 narrative moment) — §1 framing + §8
  bullet.

## What agent-15 (evidence synthesis) should do with this

1. **Treat the three structural biases (Bias-03, Bias-05, Bias-08)
   as evidential floors.** Any synthesised claim in §3 / §5 / §7
   that implicitly depends on cross-speaker, cross-encoder, or
   independently-replicated numbers must be rewritten or dropped.
2. **Preserve the AUTHOR-EMPIRICAL hedges** (V12, V17, V22 per agent-02
   glossary) on all of Case 1 / Case 2 / Case 3 derived claims.
3. **Do not synthesise** the 0.961 Case 3 SECS up into a
   population-level voice-cloning claim; the synthesised
   claim should read as "a specified-protocol, single-speaker,
   single-encoder demonstration that the 0.95 WavLM-SECS barrier
   published for closed-artefact frontier systems is exceedable on
   open scorers under the stated protocol".
4. **Treat Cluster-H industry signals as one dated narrative, not
   N-source consensus** (Bias-10).
5. **When synthesising TTS / video frontier comparisons**, pair
   Microsoft / Alibaba anchors with open-source comparators per
   the §2.5 snippets.
6. **Add no new citations.** Agent-08 master pool is LOCKED. If a
   synthesis step reveals a citation gap, raise a blocker rather
   than reaching outside the pool.

## Quality-gate status delivered

- **QG (bias coverage).** 10/10 bias axes audited; severity + scope
  + mitigation site assigned per axis. **PASS.**
- **QG (structural-vs-remediable triage).** 3 structural biases
  identified and flagged as §8-mandatory; 7 remediable biases have
  assigned caveat text. **PASS.**
- **QG (writer-ready mitigation text).** 11 copy-verbatim snippets
  keyed to specific section targets. **PASS.**
- **QG (adversarial-review prep).** 6 regex / phrase targets
  delivered for agent-42. **PASS.**

## Critical constraints carrying forward

- **Master pool LOCKED** per agent-08 — bias audit does not modify
  the pool, only the mitigation text that cites the pool.
- **Cluster labels A–I LOCKED** per agent-06.
- **Tier assignments LOCKED** per agent-07.
- **Evidence grades LOCKED** per agent-11.
- **H/W/C tier LOCKED** per agent-12.
- **Quality-flag caveat phrasing LOCKED** per agent-13.
- **Bias-mitigation snippets SOFT-LOCKED** — agents 35–40 copy
  verbatim; agent-42 may request rewrites; agent-40 (limitations-
  writer) may reorder §8 paragraphs for narrative flow but must
  preserve content.

## Open items (for agent-15 to consider or forward)

1. **Cross-encoder audit for Case 3** is named future work but could
   be surfaced in §9 alongside the replication ask. Agent-15 may
   propose a synthesis framing that makes the follow-up plan visible
   without weakening the §6.3 claim.
2. **Cluster-I scope expansion** (RLAIF / KTO / IPO / ORPO) is out of
   scope for this submission but is a natural follow-up; agent-15
   may note in §9 that the TCT contrast as framed in §5 covers the
   three canonical anchors and that extension to a broader
   alignment landscape is future work.
3. **Meta-analytic bias statistics** (Egger / trim-and-fill /
   fail-safe N) are not applicable to this heterogeneous narrative
   pool. Agent-14's audit documents *why* these tests are
   inapplicable; agent-40 (limitations) may want to include a
   one-line note in §8 that the bias apparatus used is qualitative
   / structural per PRISMA / AMSTAR-2 / ROBIS rather than
   quantitative meta-analytic, in case a reviewer asks.

## Dependencies agent-15 will pull forward

- **From agent-14 (this handoff):** bias audit + mitigation snippets.
- **From agent-13:** 4 citation-quality caveats (VALL-E 2,
  EchoMimicV3, Constitutional AI, NaturalSpeech 3).
- **From agent-12:** hot/warm/cold tier assignments.
- **From agent-11:** evidence grades per claim (STRONG /
  MODERATE / SINGLE-SOURCE / AUTHOR-EMPIRICAL / WEAK).
- **From agent-10:** theoretical-framework map.
- **From agent-09:** methodology scan.
- **From agent-08:** LOCKED master reference pool.
- **From agent-04:** construct definitions (DDA, TCT, meaning
  compression, centroid, constellation).

## Confidence

**HIGH.** Bias audit findings trace to quantified pool-composition
counts (agent-11) + measurement-protocol specifications (mission
brief §3 Cases 1–3) + citation-quality scores (agent-13). Mitigation
snippets are section-keyed, writer-ready, and align with all LOCKED
upstream decisions. The three structural biases are explicitly
owned in §8 required paragraphs; the seven remediable biases are
addressed by targeted caveat insertions across §1 through §9.

---

## Changelog

- 2026-04-17 — Initial handoff by agent-14-bias-detector.
