---
namespace: reports
agent_id: agent-29-validity-guardian
timestamp: 2026-04-16T19:00:00Z
status: active
locked: false
supersedes: none
depends_on:
  - memory/discoveries/agent-29-validity-guardian--validity-threats.md
  - memory/patterns/agent-29-validity-guardian--mitigation-plan.md
  - memory/patterns/agent-29-validity-guardian--section-8-limitations-draft.md
  - memory/handoff/agent-29-to-30--validity-ready.md
tags: final-report, smap, validity, audit-summary
---

# Agent-29 Validity-Guardian — Final Report (SMAP)

## Situation

The arXiv manuscript on Teleological Constellation Training (TCT)
makes **theoretical claims** (DDA N(N+1)/2 orthogonal-signal count,
meaning compression as fourth category, per-output verifiable
guarantee, sample-complexity collapse) and **empirical claims** from
three case studies (Case 1 Shakespeare stylistic LoRA, Case 2 Santa
talking-head clone, Case 3 voice clone with 0.961 mean WavLM SECS).

Case 3 is the only fully-measured case (0.961 mean / 0.975 max SECS on
10 held-out sentences; DNSMOS P.808 3.93/3.93 parity; UTMOS 3.012/2.997
parity). Cases 1 and 2 are T1 PENDING end-to-end. The paper defends
exploratory N = 1 demonstrations, not power-calculated population
studies.

Agent-24 surfaced 10 weaknesses (W1–W10) in the weakness register.
Agent-25 designed remediation paths. Agent-26 locked sampling
protocols with explicit exploratory-N = 1 framing. Agent-27 locked
instrument specs with byte-identical reproducibility manifests.
Agent-28 locked the analysis plan with **no hypothesis tests, no
p-values, no power calculations**, using descriptive statistics and
ablation deltas.

Agent-29's task: audit **internal, external, construct, and
statistical-conclusion validity** plus theoretical-claim validity
across all three cases and the TCT / DDA theory. Every confirmed
threat gets a mitigation and a §8 Limitations paragraph.

## Mission

Produce:
1. Threat-by-threat audit.
2. Mitigation-and-disclosure plan.
3. Paste-ready §8 Limitations draft.
4. Handoff to agent-30 (risk-analyst).
5. This final report.

## Action

- Read mission brief, TCT protocol, case-study protocols, sampling
  protocols, instrument specs, analysis plan, weakness register,
  and agent-28-to-29 handoff (seven upstream anchor files).
- Built a 31-entry validity-threat register under five frames:
  VT-I* internal, VT-E* external, VT-C* construct, VT-S*
  statistical-conclusion, VT-T* theoretical-claim.
- Classified each threat as CONFIRMED (requires disclosure / run),
  MITIGATED (design / analysis / language closes it), or
  UNCLOSEABLE (inherent to exploratory N = 1 scope).
- Mapped agent-24 W1–W10 onto the VT-* taxonomy; identified three
  additional threats beyond W1–W10 (VT-I6 forensic-fix confound;
  VT-C2 DNSMOS/UTMOS neural-proxy validity; VT-T2 threshold-
  calibration portability).
- Consolidated the three-threat headline: L-01 (encoder-matched on
  Case 3), L-02 (missing pre-selection distribution), L-05 (no
  M > 1 empirical evidence).
- Built mitigation plan stratified by Rank-1 (zero-cost language
  rewrites), Rank-2 (paste-ready disclosure paragraphs), Rank-3
  (PENDING W1/W2/W3 runs), Rank-4 (stretch v2 runs).
- Drafted §8 Limitations with L-01–L-12 assigned, mandatory vs
  enhance-if-room tagging, and two-track contingency for
  LANDED / PENDING remediation status.
- Handed off to agent-30 with explicit reviewer-rejection risk,
  reproducibility-decay risk, community-overclaim risk, vendor-
  SLA risk, and submission-timing risk.

## Product

**Five files written** (all paths absolute):

1. `/home/cabdru/contextgraph/docs/arxivpaper/memory/discoveries/agent-29-validity-guardian--validity-threats.md`
   — 31-entry threat register + cross-threat summary table.
2. `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-29-validity-guardian--mitigation-plan.md`
   — per-threat action table + paste-ready snippets + agent-42
   must-catch list.
3. `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-29-validity-guardian--section-8-limitations-draft.md`
   — paste-ready §8 draft with L-01–L-12 and summary opener.
4. `/home/cabdru/contextgraph/docs/arxivpaper/memory/handoff/agent-29-to-30--validity-ready.md`
   — handoff to risk-analyst.
5. `/home/cabdru/contextgraph/docs/arxivpaper/memory/reports/agent-29-validity-guardian--final-report.md`
   — this report.

---

## Three-sentence summary — tightest §8 disclosures

**The three threats that drive the tightest §8 disclosures, ordered
by severity of reviewer exposure if under-disclosed:**

1. **L-01 — encoder-matched evaluation on Case 3**: WavLM-base-plus-sv
   is both the Phase-3 guard (best-of-K selects by WavLM cosine ≥ 0.95)
   **and** the evaluator (the 0.961 mean SECS is measured on the same
   encoder); the reported number is the signature of the guard
   behaving as specified, not cross-encoder identity verification, and
   the paste-ready §6.3 sentence + §8 L-01 paragraph reframe the
   headline as **within-WavLM-family agreement** absent a cross-
   encoder (ECAPA-TDNN / Resemblyzer) W1 remediation run.

2. **L-02 — missing pre-selection distribution on Case 3**: we report
   the post-argmax mean 0.961 over 12 candidates per sentence but not
   the raw 120-candidate pre-selection distribution, so we cannot
   decompose the 0.961 into (a) base Qwen3-TTS + 50-clip ICL
   conditioning quality versus (b) best-of-K-under-WavLM-guard
   contribution, and §5.4 must consequently soften TCT attribution
   from "causes identity-lock" to "selects and verifies identity-
   lock over the vendor model's sample space" until the W2
   pre-selection-histogram run lands.

3. **L-05 — no M > 1 empirical evidence for multi-modal conjunction
   strictness**: H2b (multi-modal conjunction is strictly stronger
   than single-modality check) is derivationally true under Axiom 2
   at M > 1, but Case 3 is M = 1, Case 1 has a single text-style
   modality, and Case 2's four-modality conjunction accept-rate is
   T1 PENDING end-to-end, so the multi-modal paradigm rests on a
   derivational argument alone at this revision and Case 2 end-to-end
   measurement is the highest-priority future work for empirical
   validation of the multi-modal framing.

---

## Validity-frame coverage summary

| Frame | Threats | CRITICAL | HIGH | MEDIUM | LOW | Mitigation path |
|---|---|---|---|---|---|---|
| Internal | VT-I1–I6 (6) | 2 | 0 | 2 | 2 | W1 + W2 runs + §6.3 disclosure + forensic-fix note |
| External | VT-E1–E6 (6) | 1 | 3 | 2 | 0 | Disclosure-only (UNCLOSEABLE in v1) + weight-hash pinning |
| Construct | VT-C1–C7 (7) | 1 | 3 | 3 | 0 | W1 run + language discipline + paste-ready L-* paragraphs |
| Statistical-conclusion | VT-S1–S6 (6) | 1 (= W1 statistical form) | 0 | 0 | 2 | Agent-28 no-test discipline + verbatim bootstrap framing |
| Theoretical-claim | VT-T1–T7 (7) | 0 | 4 | 2 | 1 | W3 run + language tightening + RO-8 future work |

**Critical-threat concentration**: 3 of the 4 CRITICAL threats
(VT-I2 / VT-I3 / VT-C1 / VT-S4) share the same root cause — Case 3
single-encoder self-consistency combined with missing pre-selection
distribution. Two remediation runs (W1 + W2) address all three; in
their absence, two paste-ready disclosure paragraphs (§6.3 sentence +
§8 L-01 + §8 L-02) close the reviewer-exposure gap.

---

## Confidence

**HIGH** that the five-frame audit (internal / external / construct /
statistical / theoretical) exhaustively covers the validity-threat
space for this manuscript's claims and design.

**HIGH** that the three-threat headline (L-01, L-02, L-05) correctly
identifies the tightest disclosures an arXiv reviewer will probe.

**HIGH** that the §8 draft's two-track (LANDED / PENDING) contingency
structure protects submission timing regardless of whether W1 / W2 /
W3 remediation runs land by submission.

**HIGH** that statistical-conclusion validity is closed by agent-28's
no-hypothesis-test discipline with zero residual Type-I / Type-II
error inflation risk.

**MEDIUM** on whether the Rank-3 PENDING runs land pre-submission;
the plan degrades gracefully to v2 with paste-ready disclosures.

**MEDIUM** on exact §8 length allocation; agent-40 may condense
ENHANCE-IF-ROOM paragraphs to single sentences if space is tight
without loss of critical-threat disclosure.

## Changelog

- 2026-04-16 — Initial issue by agent-29-validity-guardian.
