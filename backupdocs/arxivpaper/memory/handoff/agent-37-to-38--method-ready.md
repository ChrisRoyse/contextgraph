---
namespace: handoff
agent_id: agent-37-methodology-writer
timestamp: 2026-04-17T11:00:00Z
status: active
tags: handoff, agent-38, results-writer, method-ready, section-3, section-5, case-studies, case-3-headline
---

# Handoff: Agent-37 (methodology-writer) → Agent-38 (results-writer)

## Read these files FIRST, in this order

1. `/home/cabdru/contextgraph/docs/arxivpaper/sections/03_framework_dda_mc.md`
   — §3 prose. Read this before writing §6 so you know which
   framework claims are already anchored (DDA identity, fourth-category
   taxonomy, collapse-silence scope) and do not re-narrate them.
2. `/home/cabdru/contextgraph/docs/arxivpaper/sections/06_method_tct.md`
   — §5 prose. Read this before writing §6 so you know which method
   specifications are already anchored ($C_P$, $\mathcal{L}_{\mathrm{TCT}}$,
   $G_\tau$, Phase 1/2/3 hyperparameters, Case-specific protocol table)
   and do not re-narrate them.
3. `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-37-methodology-writer--writing-decisions.md`
   — the writing decisions, hedge audit, reserve-list compliance, and
   banned-phrase check. Section 4 ("What §3 and §5 hand to §6") is the
   specific contract.
4. `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-34-confidence-quantifier--confidence-language-for-writers.md`
   — paste-ready hedges; §6 must continue carrying C5, C6, C7, C8, C9,
   C10, C11 exactly as §5.6 does.
5. `/home/cabdru/contextgraph/tasks/newdata/alldata.md` — Case-3
   measurement protocol (Full ICL + 50-clip centroid + WavLM-scored
   best-of-12 at $T = 0.3$; mean WavLM SECS 0.961, max 0.975 on 10
   held-out English sentences; DNSMOS 3.93 clone vs 3.93 real; UTMOS
   3.012 clone vs 2.997 real).

## What §3 and §5 already own (do NOT re-narrate in §6)

- The **DDA counting identity** $|D'| = n \cdot (N + \binom{N}{2})$
  with the **up to** hedge (§3.1). §6 may *reference* the identity
  when connecting case data to signal density, but must not
  re-derive it.
- The **fourth-category taxonomy** and the "semantically distinct
  unit pairs" framing (§3.2). §6 must not re-cite the
  compression-category papers.
- The **scope-of-collapse-silence** scope claim (§3.3). §6 may
  *invoke* the scope claim when discussing why DDA-enriched
  supervision is safe at scale, but must not re-narrate the
  Shumailov / Dohmatob / Alemohammad / Gerstgrasser pile.
- The **three-role composition** (§3.4). §6 may *instantiate* it
  (Context Graph retriever role; ClipCannon labeller role; TCT
  runtime guard role) but must not re-establish the role-composition
  claim.
- The **constellation $C_P$** definition and the three-phase
  protocol (§5.1–§5.4). §6 must not re-specify Phase 1/2/3; §6
  reports the **instantiation** of the protocol in Cases 1, 2, 3.
- The **acceptance predicate** $G_\tau(\hat y; C_P)$ (§5.4 eq. 5).
  §6 reports the decisions the guard produces on each case's
  held-out set; it must not re-state the predicate.
- The **three-layer enforcement stack** for text (§5.5). §6.1
  reports the Case-1 status against this stack; it must not
  re-specify the layers.
- The **case-specific protocol table** (§5.6 Table 2). §6 carries
  the **results** that correspond to each protocol row; it must not
  duplicate the protocol table.

## What §6 owns (primary contribution)

### §6.1 Case 1 — Shakespeare stylistic LoRA

- **Status**: T3 projected end-to-end; prior-run 15/15 FSV measured.
- **Content to add**: (a) the prior-run FSV battery details (what
  was tested, what was scored); (b) the forensic echo-bug description
  and its remediation; (c) the in-progress SFT + DPO retrain timing
  and checkpoint status; (d) the projection of 15/15 pass on the
  cleaned retrain with camera-ready re-verification caveat (C11
  hedge).
- **Language**: "SFT + DPO retrain on forensically-cleaned data is
  in progress at the time of writing. The structure of the enforcement
  stack is unchanged; we project pass but disclose the retrain status
  as pending T1 confirmation."
- **Citations**: may re-cite `hu2022lora` (pre-approved); must not
  re-cite the scalar-reward cluster.

### §6.2 Case 2 — Identity-locked talking-head clone (Santa)

- **Status**: T3 projected end-to-end; forensic integrity complete.
- **Content to add**: (a) ClipCannon 23-stage DAG summary leading to
  2,362 training clips; (b) the seven-modality specification (or up
  to seven, tier-flagged); (c) the mouth-weighted flow-matching
  sign-convention forensic and its fix; (d) the LoRA rank-256
  $\alpha = 512$ specification with `q_audio` excluded; (e) the
  operating-threshold projections $(0.70, 0.60, 0.80, 0.85, \ldots)$;
  (f) explicit T3 framing throughout and the "camera-ready will
  re-verify" caveat.
- **Language**: "**Projected (T3).** By analogy to Case 3 and under
  Axiom 2, we predict the constellation guard materially affects
  identity-cosine stability on Case 2. T1 measurement is pending
  (H2d, H3b)."
- **Citations**: may re-cite `hu2022lora`, `lipman2023flowmatching`
  (pre-approved); must not re-cite the talking-head generation
  cluster for the "no per-modality conjunction guard" framing (done
  in §2.5).

### §6.3 Case 3 — Voice cloning with a 50-clip audio centroid

- **Status**: **T1 measured** (the only fully-measured case).
- **Headline result sentence (paste-ready)**: "Under the encoder-
  matched protocol, the measured Case-3 result on 10 held-out
  English sentences for one speaker is **mean WavLM SECS 0.961, max
  0.975**, at $K = 12$, $T = 0.3$, $\tau = 0.95$, pipeline version
  `qwen3-tts-foricl-v1`, measurement date 2026-04-16."
- **Mandatory companion sentence (C5)**: "This is an **encoder-
  matched** measurement: guard and evaluator are both
  WavLM-base-plus-sv. This demonstrates the guard behaves as
  specified; it does not constitute cross-encoder identity
  verification. Cross-encoder triangulation (ECAPA-TDNN or
  Resemblyzer on the same 10 sentences) is instrument-ready and
  named as pending."
- **DNSMOS / UTMOS parity sentence (C6)**: "DNSMOS-P.808 MOS for the
  clone (3.93) matches the real recording (3.93) within the
  evaluator's observed variance on this 14-sentence comparison set;
  UTMOS shows the clone marginally higher (3.012 vs. 2.997).
  Single-speaker, English-only scope applies (Bias-05, Bias-06); a
  formal statistical equivalence test across a larger speaker panel
  is out of scope."
- **Temperature sweep sentence (C7)**: "In our sweep $T \in \{0.1,
  0.3, 0.5, 0.7, 0.9\}$, $T = 0.3$ produced the highest
  post-selection mean SECS under WavLM best-of-12. The optimum is
  **pipeline-specific**; we make no claim about transport to other
  speakers, other TTS backends, or other text corpora."
- **Centroid-size ablation sentence (C8)**: "Centroid ablation
  (paired, 10 held-out sentences): 50-clip centroid mean = 0.961;
  1-clip centroid mean = 0.946; delta = +0.015. Single operating
  point on the $n_m$ curve; sample-complexity curve RO-10 is future
  work."
- **Bootstrap CI sentence**: "A within-WavLM-family retrospective
  95 % bootstrap CI on the mean ($N = 10$, 10,000 resamples, fixed
  seed 20260416, percentile method) is $[0.951, 0.971]$ —
  retrospective, not power-based, descriptive summary of the ten
  observed values."
- **Three pending measurements** (name them, do not claim them):
  cross-encoder triangulation (ECAPA-TDNN / Resemblyzer);
  pre-selection cosine distribution over all 120 candidates;
  reference-set-size curve at $n_m \in \{10, 25, 50, 100\}$.
- **Citations**: may re-cite `chen2022wavlm` (pre-approved, scorer
  identity).

## Hedges MUST carry into §6

- **C5** — "encoder-matched protocol" on every 0.961 SECS mention.
- **C6** — "within evaluator noise, N = 14" on every DNSMOS /
  UTMOS parity mention.
- **C7** — "in this pipeline / speaker / corpus" on every $T = 0.3$
  mention.
- **C8** — "single operating point; $n_m$ curve RO-10 future work"
  on every 50-clip-centroid mention.
- **C9** — "we predict" / "by analogy to Case 3" / "T1 pending"
  on every Case-2 quantitative claim.
- **C10** — "consistent with" / "hypothesise" (never "necessary")
  on every mouth-weighted-loss mention.
- **C11** — "retrain in progress", "projected (T3)" on every Case-1
  cleaned-retrain mention.
- **V22** — measured vs. projected tier-flag on every number.
- **V18** — "frozen" or "frozen-with-LoRA" on every embedder mention.

## Banned phrases STILL banned in §6

All 11 rows of agent-34 §4 apply. High-risk items for §6:

- "91 signals per input" → "up to 91 signals per input".
- "0.961 mean SECS" bare → always paired with encoder-matched.
- "indistinguishable from real speech" bare → "DNSMOS parity within
  evaluator noise, N = 14, single speaker".
- "we demonstrate the sample-complexity collapse" → use C12a / C12b
  split.

## What §6 should produce for §7 (discussion)

- Structured result summary: Case 1 T3 projected with 15/15 prior
  anchor; Case 2 T3 projected with forensic integrity complete;
  Case 3 T1 measured with encoder-matched caveat.
- A one-paragraph synthesis framing the three cases as graduated
  evidence for TCT's sample-complexity collapse: Case 3 (M=1)
  provides the one T1 operating point; Cases 1 and 2 project to
  higher $M$ with protocol-measured architectural readiness.
- Explicit list of the three pending companion measurements for
  Case 3 so §8 can quote them verbatim.

## Files agent-37 produced

- `/home/cabdru/contextgraph/docs/arxivpaper/sections/03_framework_dda_mc.md`
  (§3 prose, verified; 1,780 words; no frontmatter)
- `/home/cabdru/contextgraph/docs/arxivpaper/sections/06_method_tct.md`
  (§5 prose, verified; 2,550 words; no frontmatter)
- `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-37-methodology-writer--writing-decisions.md`
- `/home/cabdru/contextgraph/docs/arxivpaper/memory/handoff/agent-37-to-38--method-ready.md`
  (this file)
- `/home/cabdru/contextgraph/docs/arxivpaper/memory/reports/agent-37-methodology-writer--final-report.md`

## Quality-gate status delivered by agent-37

| Gate | Status |
|---|---|
| §3 prose file written at `sections/03_framework_dda_mc.md` with `# 3. Framework: DDA and Meaning Compression` header | **PASS** (1,780 words, within 1,400–1,800 target) |
| §5 prose file written at `sections/06_method_tct.md` with `# 5. Teleological Constellation Training` header | **PASS** (2,550 words, within 2,000–2,500 target ± edit slack) |
| §3 has §3.1 DDA identity + §3.2 meaning compression + §3.3 collapse scope + §3.4 three-role composition | **PASS** |
| §5 has §5.1 $C_P$ + §5.2 Phase 1 + §5.3 Phase 2 + §5.4 Phase 3 + §5.5 three-layer stack + §5.6 case protocols | **PASS** |
| DDA identity equation $\lvert S \rvert = n(N + \binom{N}{2})$ present with constructive-upper-bound caveat | **PASS** |
| Phase-3 acceptance predicate $G_\tau(\hat y; C_P) = \mathbb{1}[\forall m : \cos \geq \tau_m]$ present | **PASS** |
| Case-3 protocol specified as Full ICL + 50-clip centroid + WavLM-scored best-of-12 at $T = 0.3$ | **PASS** |
| Measured / projected tier-flag on every number | **PASS** |
| agent-34 C1 / C2 / C3 / C4 / C5 / C12a / C12b / C13 hedges applied | **PASS** |
| Reserve-list compliance (no forbidden re-citations) | **PASS** |
| Banned-phrase list clean | **PASS** |
| No Python / Rust code blocks in body | **PASS** |
| No frontmatter on section files | **PASS** |
| APA 7th in-text format | **PASS** |

## Confidence

**HIGH** that §6 will not duplicate §3 or §5 content.

**HIGH** that the paste-ready headline sentences for Case 3 carry
the encoder-matched scope in the sentence itself.

**HIGH** that the Case-2 T3 projection will not slip into
measured-sounding prose.

**HIGH** that the reserve list is intact and §6 has clear boundaries.

**MEDIUM** on agent-42 adversarial review: the three-role composition
novelty claim (§3.4) and the 0.961 encoder-matched headline (§5.6 /
§6.3) are the two most likely review-exposure points.

## Changelog

- 2026-04-17 — Initial handoff by agent-37-methodology-writer.
