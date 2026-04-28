---
namespace: reports
agent_id: agent-09-methodology-scanner
timestamp: 2026-04-16T19:00:00Z
status: active
tags: smap, final-report, methodology-scanner, handoff-agent-10
---

# SMAP Final Report — Agent-09 Methodology Scanner

## S — Situation

The pipeline entered methodology scanning with:
- **88 citations** consolidated and LOCKED by agent-08 (9 clusters A–I; 85%
  T1+T2 in the core A–G+I subset).
- **18 per-cluster structural holes** from agent-06, consolidated into **5
  combined HOLEs** by agent-06's structural-holes.md.
- **20 essential questions** (Q01–Q20) with body-pool cite targets from
  agent-08's claim-to-citation index.
- **Three empirical case studies** (mission brief §3): Shakespeare LoRA (T3),
  Santa talking-head (T2+T3), Voice cloning (T1 — the only fully-measured case).
- **Two production systems** (mission brief §2): Context Graph (N=13 embedders),
  ClipCannon (N=7 modalities).
- **Four Tier-A constructs** (DDA, meaning compression, TCT, identity-locked
  generation) that are load-bearing across the manuscript.

No methodology taxonomy existed; no per-cluster metric inventory existed; no
method-theory alignment check had been run; no per-case-study comparator-method
cheatsheet had been written.

## M — Mission

Per agent-05 tasking table row 09: scan the 88-citation master pool for
methodological traditions, standard metrics, method-theory alignment, and
methodological gaps; produce a per-cluster analysis (A–I); produce a single-page
cheatsheet for agent-37 (methodology-writer) for Cases 1 / 2 / 3; hand off to
agent-10 (theoretical-framework-analyst) with a coherent methodology map.

Quality gates targeted:
- **QG2 G9** — method-theory consistency check per cluster.
- **QG2 G10** — comparator-method completeness per case study.
- **FM-03** — no AGI overclaim; every gap framed as structural delta.
- **FM-10** — no citation padding; every gap evidence-grounded in specific
  papers.

## A — Actions

1. **Read all locked upstream files:** mission brief, canonical glossary,
   definition blocks (agent-04), topic topology (agent-06), structural holes
   (agent-06), master reference pool (agent-08), claim-to-citation index
   (agent-08), unresolved citations (agent-08), agent-08-to-09 handoff.
2. **Scanned all 88 citations across nine clusters A–I**, classifying each by a
   9-code methodological tradition scheme (EB, TD, FC, AB, MA, SR, SI, SA, CT)
   and reading the method (not the result) for each.
3. **Extracted per-cluster dominant traditions, metrics, method-theory
   alignment, and per-cluster methodological gaps.** 23 per-cluster gaps
   identified.
4. **Consolidated 23 per-cluster gaps into 9 methodological gaps** that our
   paper explicitly fills, each traced to a structural HOLE-1…HOLE-5, a Tier-A
   construct, and an essential question Q01–Q20.
5. **Built the standard-metrics-per-cluster inventory** with unit / direction /
   reporting context / canonical phrasing / case-study applicability.
6. **Built the per-case methods cheatsheet** for agent-37 (methodology-writer),
   agent-38 (results-writer), and agent-39 (discussion-writer). Per-case:
   comparator keys, metrics to report, ablations to run, novelties to claim,
   forbidden phrasings per V-rules.
7. **Wrote the agent-10 handoff** with all five output files cross-linked, plus
   three open theoretical questions flagged for agent-10's framework design.
8. **Wrote this SMAP report.**

No citations were added. No blockers raised. U01 (Qwen3-TTS) continues forward
from agent-08 and must be resolved before §6.3 finalisation.

## P — Products (files written)

All paths absolute:

1. `/home/cabdru/contextgraph/docs/arxivpaper/memory/discoveries/agent-09-methodology-scanner--methodology-scan.md`
   — **per-cluster methodology analysis** (nine clusters A–I, 23 per-cluster
   gaps, method-theory alignment per cluster).

2. `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-09-methodology-scanner--standard-metrics-per-cluster.md`
   — **metric inventory** with canonical phrasing, case-study applicability,
   mandated reporting checklists.

3. `/home/cabdru/contextgraph/docs/arxivpaper/memory/discoveries/agent-09-methodology-scanner--methodological-gaps.md`
   — **9 consolidated methodological gaps** our paper fills, each traced to
   HOLE / Tier-A construct / Q-ID.

4. `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-09-methodology-scanner--methods-cheatsheet-for-writers.md`
   — **per-case comparator cheatsheet** for agent-37 + agent-38 + agent-39.

5. `/home/cabdru/contextgraph/docs/arxivpaper/memory/handoff/agent-09-to-10--methodology-mapped.md`
   — **handoff to agent-10** (theoretical-framework-analyst) with three open
   theoretical questions flagged.

6. `/home/cabdru/contextgraph/docs/arxivpaper/memory/reports/agent-09-methodology-scanner--final-report.md`
   — **this SMAP report**.

## Key outputs at a glance

### 9-code methodological tradition scheme

EB (empirical benchmark), TD (theoretical derivation), FC (forensic case study),
AB (ablation study), MA (meta-analysis), SR (systematic review / position),
SI (statistical / industry signal), SA (system-artefact release), CT (controlled
trial with named protocol).

### Per-cluster dominant traditions

- **A** TD + SI, **B** TD + EB, **C** EB + MA, **D** EB + AB + TD, **E** EB + AB
  + CT, **F** EB + AB + TD, **G** heterogeneous, **H** SI + EB, **I** EB + AB + TD.

### 9 consolidated methodological gaps our paper fills

1. Signals-per-input is not a measurement convention (A, C, D).
2. No cross-embedder pairwise-interaction measurement (C, D).
3. No multi-clip centroid aggregation as alignment target (E, F).
4. No runtime per-output cosine-gate against frozen centroid (D, E, F, I).
5. No cross-encoder SECS protocol (E).
6. No WavLM SECS same-session-band precedent (E).
7. Flow-matching sign-convention forensic undocumented (F).
8. Shakespeare echo-bug forensic undocumented (I).
9. Sample-complexity under TCT unmeasured in cited work (E, F, I).

### The singular cross-cluster novelty

**The constellation guard** (METHOD-GAP-4) is methodologically novel across
**four clusters simultaneously** (D retrieval, E TTS, F video, I alignment).
This is the paper's strongest methodological contribution across the entire
cited literature and is the inference-time half of TCT (glossary #3 / #12).

## Key methodological observations our paper must defend

- **Case 3 mean 0.961 WavLM SECS** is measured; the same-session band (0.95–0.99)
  is authors' empirical observation with V12 hedge required on first use.
- **Cross-encoder SECS** (Qwen3-TTS conditioning; WavLM-SV scoring; different
  architectural families) eliminates the scoring-with-your-own-training-signal
  bias — but §8 Limitations must preempt the "VALL-E 2's SIM-o already qualifies"
  objection explicitly.
- **Flow-matching sign convention** (target = noise − latent) must be stated on
  first use in §5 / §6.2; the forensic file path disclosed per P10.
- **Echo-bug forensic** for Case 1: 15/15 FSV pass on cleaned data; disclose
  diagnostic method + fix + post-fix verification.

## Ablations required across cases

- **Case 1:** SFT-only vs. SFT+DPO; original vs. cleaned data; guard present
  vs. absent (T3 projected).
- **Case 2:** Centroid present vs. absent; mouth-weighted vs. uniform
  flow-matching; per-modality τ_m sweep; (optional negative control) flipped
  flow-matching sign.
- **Case 3:** N=1 vs. N=50 reference; no-guard vs. guarded; T=0 greedy vs.
  T=0.3 best-of-K (K ∈ {1, 4, 12}); (T3 optional) cross-encoder generalisation
  with ECAPA-TDNN.

## Quality gate status

| Gate | Requirement | Status | Notes |
|---|---|---|---|
| **QG2 G9** (method-theory consistency per cluster) | All clusters' cited methods align with their theoretical traditions | **PASS** | Cluster G heterogeneous by design; not a misalignment |
| **QG2 G10** (comparator method completeness per case) | Every case study has named comparator methods from master pool | **PASS pending U01** | Qwen3-TTS citation not yet entered |
| **FM-03** (no AGI overclaim) | Every gap is a structural delta, not a superlative | **PASS** | All 9 gaps phrased as "the cited cluster does not include X; we add X" |
| **FM-10** (no citation padding) | Every gap is traced to specific papers whose methodology lacks the feature | **PASS** | 9 gaps × 2–5 papers each |

## Unresolved items carrying forward

- **U01 Qwen3-TTS** (from agent-08). Base model for Case 3. Must be entered at
  T3 via GitHub disclosure before §6.3 finalisation.
- **METHOD-GAP-5 cross-encoder SECS**. Agent-11 should verify whether any
  Cluster-E paper implicitly enforces cross-encoder independence (possibly
  chen2024valle2's SIM-o qualifies incidentally). If so, demote from HIGH to
  MEDIUM confidence and update §8 Limitations.
- **Theoretical framework for constellation-distance loss vs. implicit reward**
  (flagged to agent-10). Open whether we anchor to prototypical network /
  energy-based-model theory or leave as §10 open work.

## 3-sentence summary for the coordinator

1. **Methodology mapped.** Nine citation clusters (A–I; 88 papers) classified
   by method; per-cluster standard metrics and method-theory alignment
   documented; 9 consolidated methodological gaps traced to the 5 combined
   structural holes and to the 4 Tier-A constructs — the strongest gap fill is
   the **constellation guard** (runtime per-output cosine-gate), methodologically
   novel across retrieval + TTS + video + alignment clusters simultaneously.

2. **Metrics our paper must report per case.** Case 3 (T1 measured): WavLM SECS
   mean/max, DNSMOS P.808 parity, UTMOS parity, same-session band as authors'
   observation, plus ablations over reference-set size (N=1 vs N=50) and guard
   (best-of-K at T=0.3). Case 2 (T2+T3 projected): seven-modality pipeline
   counts + projected per-modality cosines + LSE-D/LSE-C/FVD/CSIM + flow-matching
   sign-convention forensic. Case 1 (T2+T3 projected): SFT+DPO metrics +
   echo-bug forensic (15/15 FSV post-fix) + projected text-modality guard
   accept-rate.

3. **Every metric must use canonical phrasing** (V4 WavLM SECS never bare;
   V12 same-session band hedged as authors' observation; V22 every projected
   number flagged), and every methodological novelty claim in §5/§6 must trace
   to one of the 9 gaps and be defended with an ablation from the cheatsheet.

## Confidence

**HIGH** on method classification, per-cluster gap identification, metric
inventory, and per-case-study cheatsheet.

**MEDIUM-HIGH** on METHOD-GAP-5 (cross-encoder SECS) — pending agent-11
verification that no Cluster-E paper incidentally qualifies.

**MEDIUM** on METHOD-GAP-9 (sample complexity under TCT) — Case 3 provides a
strong point datum (50 clips → 0.961) but a full sample-complexity curve is open
work for §10.

---

## Changelog

- 2026-04-16 — Initial SMAP report by agent-09-methodology-scanner.
