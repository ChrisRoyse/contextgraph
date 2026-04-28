---
namespace: reports
agent_id: agent-14-bias-detector
timestamp: 2026-04-17T00:10:00Z
status: active
tags: smap, final-report, bias-audit, ten-axes, structural-vs-remediable, mitigation-snippets
---

# Agent-14 Bias Detector — SMAP Final Report

## S — Summary

Agent-14 audited **ten bias axes** (per task brief) across (a) the
LOCKED 88-citation master reference pool and (b) the paper's own
empirical claims in Case 1 (Shakespeare), Case 2 (Santa, n = 1 subject),
and Case 3 (voice, author-as-subject, WavLM-only).

- **All 10 bias axes triaged** with severity (CRITICAL / HIGH /
  MODERATE / LOW) and scope (pool / empirical claims / both).
- **Three biases are CRITICAL and STRUCTURAL**: Bias-03 industry-lab /
  affiliation concentration (Microsoft × 4 in Cluster E, Alibaba × 4
  in Cluster F, US-only × 3 in Cluster I), Bias-05 single-subject
  (Cases 2, 3), and Bias-08 self-reporting (all three cases).
- **These same three require dedicated §8 Limitations paragraphs.**
  Three copy-verbatim paragraphs delivered.
- **Seven biases are remediable by writing discipline**: Bias-01
  (publication bias in TTS/video), Bias-02 (citation selection),
  Bias-04 (English-only), Bias-06 (WavLM-only encoder), Bias-07
  (forensic confirmation / retcon), Bias-09 (LibriSpeech / VoxCeleb
  benchmark monoculture), Bias-10 (temporal 2024Q4 narrative
  moment). Eight additional copy-verbatim snippets delivered for
  §1 / §2.5 TTS / §2.5 video / §2.6 / §6.1 / §6.2 / §6.3 / §7 / §9 /
  abstract.
- **Six agent-42 adversarial-review regex targets** delivered.
- **No pool modification**; all agent-06 / 07 / 08 / 11 / 12 / 13
  LOCKED decisions preserved.

## M — Method

1. Read mission brief, agent-01 failure modes, agent-11 cluster-level
   bias audit, agent-11 per-claim evidence grades, agent-13 quality
   flags, and agent-13-to-14 handoff.
2. For each of the 10 task-brief bias axes, documented:
   - **What it is** (meta-science definition).
   - **Is it present?** YES / PARTIAL / NO with severity.
   - **Evidence** quantified from (a) pool counts and cluster
     composition (agent-11), (b) citation rubric scores (agent-13),
     and (c) measurement-protocol specifications (mission brief §3).
   - **Scope** — pool / empirical claims / both.
   - **Mitigation** — concrete section-keyed action with writer-
     facing snippet text.
3. Distinguished **STRUCTURAL** biases (Bias-05, Bias-06, Bias-08 —
   cannot be fixed within this submission; must be owned) from
   **REMEDIABLE** biases (Bias-01, 02, 04, 07, 09, 10 — fixed by
   writing discipline and citation balance). Bias-03 sits between:
   field-wide and partially structural, but the *pool's* Microsoft /
   Alibaba / US-only concentration is actually mirroring the field
   honestly — the affiliation tilt is owned by §2.5 pairing plus
   §8 paragraph.
4. Collapsed the 10-axis audit into **11 copy-verbatim writer
   snippets** section-keyed to §1 / §2.5 / §2.6 / §6.1 / §6.2 /
   §6.3 / §7 / §8 / §9 / abstract.
5. Noted that meta-analytic bias tests (Egger / trim-and-fill /
   fail-safe N) are **not applicable** to this heterogeneous
   narrative pool (9 clusters × heterogeneous outcomes); the
   appropriate apparatus is qualitative / structural per PRISMA /
   AMSTAR-2 / ROBIS, which is what the audit uses.
6. Wrote two memory files (discoveries + patterns), one handoff, and
   this SMAP.

## A — Artefacts

1. **`memory/discoveries/agent-14-bias-detector--bias-audit.md`** —
   full 10-axis audit with per-axis evidence, severity, scope, and
   mitigation.
2. **`memory/patterns/agent-14-bias-detector--bias-mitigations.md`**
   — 11 copy-verbatim mitigation snippets + agent-42 regex targets
   + writer checklist covering §1 through §9.
3. **`memory/handoff/agent-14-to-15--biases-documented.md`** —
   handoff to evidence-synthesizer with structural-vs-remediable
   triage and explicit instruction on how synthesis must respect
   bias scope.
4. **`memory/reports/agent-14-bias-detector--final-report.md`** —
   this SMAP.

## P — Provenance / next agent

**Next agent:** agent-15-evidence-synthesizer.

**What agent-15 receives:**
- 10-axis bias audit with severity + scope + mitigation.
- 11 writer-ready mitigation snippets.
- Three CRITICAL structural biases flagged for §8 required paragraphs.
- Agent-42 adversarial regex targets.
- Writer-facing §1–§9 checklist.

**What agent-15 must NOT do:**
- Synthesise around the structural biases (Bias-05, Bias-06,
  Bias-08). Cross-speaker, cross-encoder, and independently-
  replicated claims must be rewritten or dropped.
- Add new citations. Master pool LOCKED.
- Upgrade AUTHOR-EMPIRICAL claims (CL-16, CL-17, CL-18) to
  consensus / STRONG grade.

**Quality gates passed:**
- Bias coverage (10/10 axes).
- Structural-vs-remediable triage.
- Writer-ready mitigation text.
- Adversarial-review preparation.

## Key findings in one paragraph

The three biases that will every reviewer will raise first are
**industry-lab / affiliation concentration**, **single-subject
empirical scope**, and **self-reporting of all case numbers**. The
first is a field-wide pattern our pool mirrors honestly and is
mitigated by pairing closed-artefact industry citations with
open-source comparators plus a dedicated §8 paragraph. The second
and third are structural features of our empirical artefacts
(n = 1 for Cases 2, 3; author is the developer and the subject) and
are mitigated only by explicit scope statements in §6.2 / §6.3 /
§8 and by naming third-party replication + cross-encoder audit as
§9 follow-up. Seven additional biases (publication, citation
selection, English-only, single-encoder, forensic retcon,
benchmark monoculture, temporal 2024Q4 narrative compression) are
remediable by careful writing and citation balance, with
copy-verbatim snippets delivered for each target section.

## Confidence

**HIGH.** All bias findings are grounded in quantified pool
statistics (agent-11 cluster counts), citation rubric scores
(agent-13), and measurement protocol specifications (mission brief
§3). All mitigation snippets are section-keyed and ready for
writer copy-verbatim use. The structural-vs-remediable distinction
is defensible: the three structural biases are genuine design
features of the submitted artefacts, not construction defects, and
the mitigation is disclosure, not remediation.

## Changelog

- 2026-04-17 — Initial issue by agent-14-bias-detector.
