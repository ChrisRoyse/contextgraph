---
namespace: handoff
agent_id: agent-09-methodology-scanner
timestamp: 2026-04-16T18:55:00Z
status: active
tags: handoff, agent-10, theoretical-framework-analyst, methodology-mapped
---

# Handoff: Agent-09 (methodology-scanner) → Agent-10 (theoretical-framework-analyst)

## Read these files FIRST, in this order

1. `./memory/discoveries/agent-09-methodology-scanner--methodology-scan.md` —
   **per-cluster methodology analysis**. Nine clusters A–I; each carries dominant
   tradition code (EB/TD/FC/AB/MA/SR/SI/SA/CT), standard metrics, method-theory
   alignment, per-cluster methodological gaps, and how our three cases engage.
2. `./memory/patterns/agent-09-methodology-scanner--standard-metrics-per-cluster.md` —
   **writer-ready metric inventory**. Canonical phrasing + case-study
   reporting checklists.
3. `./memory/discoveries/agent-09-methodology-scanner--methodological-gaps.md` —
   **9 consolidated methodological gaps** our paper fills, each traced to
   Tier-A constructs, Q-IDs, and HOLE-1 … HOLE-5 from agent-06.
4. `./memory/patterns/agent-09-methodology-scanner--methods-cheatsheet-for-writers.md` —
   **single-page cheatsheet for agent-37**. Per-case comparator methods,
   metrics, ablations, novelty claims. Forbidden phrasings flagged per V-rules.
5. `./memory/reports/agent-09-methodology-scanner--final-report.md` — SMAP
   report.

## What agent-09 did

1. **Scanned all 88 citations** in the master pool across nine clusters A–I.
2. **Classified each citation by methodological tradition** using a 9-code scheme
   (EB / TD / FC / AB / MA / SR / SI / SA / CT).
3. **Mapped standard evaluation metrics** per cluster with unit / direction /
   reporting context / whether our paper uses it.
4. **Assessed method-theory alignment** per cluster (is the cited method
   appropriate for the cluster's theoretical claims).
5. **Identified 23 per-cluster methodological gaps**, consolidated into **9
   methodological gaps** that our paper explicitly fills, each traced to a
   structural hole, a Tier-A construct, and an essential question.
6. **Built the per-case-study methods cheatsheet** for agent-37: which cluster
   methods to benchmark against in Cases 1 / 2 / 3, which metrics to report in
   canonical phrasing, which ablations to run, which methodological novelties
   to claim.
7. **Did NOT add citations.** All comparators are drawn from the LOCKED master
   pool; Qwen3-TTS remains the single unresolved key (U01 from agent-08).

## Headline outcomes

- **Methodology-covered clusters.** All nine clusters A–I — each carries a
  dominant tradition (EB/AB typical in D/E/F; TD+EB in A/B/C; SI+EB in H;
  EB+AB+TD in I).
- **Method-theory alignment.** Tight within clusters A/B/C/D/E/F/I; heterogeneous
  in G (glue) and H (market signals). No cluster flagged as "misaligned
  method-theory."
- **Methodological gaps filled by our paper: 9** (consolidated from 23 per-cluster
  findings).
- **Novelties methodologically cross-cluster.** Constellation guard (METHOD-GAP-4)
  is novel across 4 clusters (D + E + F + I). Centroid aggregation (METHOD-GAP-3)
  is novel across 2 (E + F). Signals-per-input metric (METHOD-GAP-1) is novel
  across 3 (A + C + D).
- **Metric inventory is writer-complete.** Every metric listed for Cases 1 / 2 / 3
  is either (a) measured and reportable now (Case 3 T1), or (b) pipeline-counted
  (Case 2 T2), or (c) flagged T3 projected with V22.

## Quality gate status

- **QG2 (Method-theory consistency) G9 (method alignment).** Agent-09's
  contribution: **PASS.** Every cluster's dominant method matches its theoretical
  tradition; our paper's methods are traced to an appropriate cluster's
  methodology and differ from it only by named methodological gaps.
- **QG2 G10 (comparator method completeness).** Agent-09's contribution: **PASS
  pending U01 Qwen3-TTS resolution.** Cases 1 and 2 comparator methods are fully
  specified; Case 3 is fully specified except for the Qwen3-TTS base-model
  citation.
- **QG1 G8 (tier coverage).** Already passed by agent-08 at 85%; agent-09 did
  not change tier assignments.

## What agent-10 (theoretical-framework-analyst) should do next

Per agent-05 tasking-table row 10:

1. **Read all five files above in order.** The methodology scan IS the evidence
   base for the theoretical-framework analysis — a theoretical framework must
   be aligned with a coherent methodology.
2. **Build the paper's theoretical framework map.** For each of the four Tier-A
   constructs (DDA, meaning compression, TCT, identity-locked generation),
   identify:
   - The theoretical foundation (which cluster's theory underpins this
     construct: compression-as-intelligence, geometric alignment, scaling laws,
     model-collapse conditions, etc.).
   - The theoretical novelty-delta: what theoretical claim does our paper make
     that the cited clusters' theories do not?
   - The method-theory alignment check: does our proposed methodology (from
     agent-09) match the theoretical framework (to be built by agent-10)?
3. **Identify theoretical gaps.** Not methodological gaps (those are agent-09's
   output) but theoretical ones — e.g., "no cited cluster provides a theory
   for why frozen-embedder decomposition yields approximately independent
   signals" (this is V14 / the #14 glossary hedge; agent-10 may wish to deepen
   it or leave it as an explicit open question for §10).
4. **Flag any method-theory mismatches.** If agent-10 finds that our proposed
   method (agent-09) conflicts with the theoretical framework being built,
   **file a blocker** before the PRISMA systematic review (agent-11) runs.
5. **Do NOT add citations.** If a theoretical anchor is needed beyond the
   master pool, file a blocker.

## What agent-11 (systematic-reviewer) should do later

- Review the nine methodological gaps (methodological-gaps.md) and check
  whether any PRISMA expansion of a specific cluster is warranted to strengthen
  the gap evidence.
- Specifically verify METHOD-GAP-5 (cross-encoder SECS) — a competent Cluster-E
  reviewer may argue VALL-E 2's SIM-o already qualifies; agent-11 should check
  whether an explicit cross-encoder definition exists in the cited Cluster-E
  papers.
- Verify METHOD-GAP-6 (same-session band) — no published reference is expected,
  but check whether any speaker-verification paper has reported a within-session
  band that would anchor the 0.95–0.99 claim as a published observation.
- If PRISMA expansion is warranted, file a blocker before updating the pool.

## What writers (agents 36–41) must do

- **agent-36 Related Work.** Use the novelty-delta phrasing in
  structural-holes.md + methodological-gaps.md to build the §2 G3 novelty
  table. Every novelty row traces to a METHOD-GAP and a HOLE.
- **agent-37 Methodology.** Use the methods cheatsheet strictly. Cite by key.
  Every method element maps to a citation key. Every novelty claim maps to a
  METHOD-GAP.
- **agent-38 Results.** Use the metrics inventory strictly. Canonical phrasing
  (V4, V12, V20). Report-checklist per case study.
- **agent-39 Discussion.** Use the method-theory-alignment row ("how our cases
  fit") as the anchor for the §7 Discussion paragraphs on positioning.
- **agent-42 Adversarial reviewer.** Your check: every metric named in §5 / §6
  methods must appear in results. Every novelty claim must trace to a
  METHOD-GAP. Every forbidden phrasing (V1–V22) must be absent.

## Critical constraints carrying forward

- **Do not add citations without a blocker.** Master pool LOCKED.
- **Do not contradict methodological classifications** without agent-09
  re-scanning.
- **U01 Qwen3-TTS must be resolved** before §6.3 Methods is finalised (carries
  forward from agent-08 handoff).
- **V12 same-session-band hedge** must appear on first use of 0.95 / 0.975 /
  0.961 / same-session in §5 and §6.3.
- **V22 T3-projected flag** must appear on every unmeasured number in Case 1
  and Case 2.
- **Every methodological novelty claim in §5 / §6 maps to a METHOD-GAP** in
  methodological-gaps.md. Adversarial-reviewer check at §42 enforces this.

## Open items (for agent-10 to consider)

1. **Theoretical framework for N-embedder orthogonality.** Glossary #14 hedges
   "approximately independent; strict formal orthogonality is an open question
   (§10)". Agent-10 may wish to select a theoretical anchor (information theory?
   independent subspace analysis? representation alignment literature?) or
   leave as §10 open work.
2. **Theoretical framework for TCT vs. RLHF equivalence.** §5 vs. §2.6 needs a
   theoretical row that states precisely how constellation-distance loss
   relates (or does not relate) to an implicit reward function. Agent-10 may
   frame this in terms of reward-as-distance (prototypical network lineage?
   energy-based model lineage?) or leave as §10 open work.
3. **Theoretical framework for meaning compression as a distinct category.**
   §3.1 makes the dimensional-orthogonality argument (different unit column:
   signals/input vs. bits/bits vs. accuracy/params vs. memory/quality). Agent-10
   may wish to deepen this into a unit-analytic argument or leave as §3.1 stand.

## Confidence

**HIGH.** Every citation in the master pool has been read for its methodology;
every cluster's dominant tradition is classified; every gap is evidence-grounded
in specific cited papers; every novelty-delta is traced to a Tier-A construct
and a structural hole. The per-case cheatsheet for agent-37 is complete and
actionable. One must-resolve (U01 Qwen3-TTS) is flagged from upstream and
continues forward.

---

## Changelog

- 2026-04-16 — Initial handoff by agent-09-methodology-scanner.
