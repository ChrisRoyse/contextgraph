---
namespace: discoveries
agent_id: agent-33-falsification-agent
timestamp: 2026-04-17T07:50:00Z
status: active
locked: false
supersedes: none
tags: falsification, killer-experiments, empirical-tests, decision-criteria, cost-analysis
depends_on:
  - memory/decisions/agent-21-theory-builder--theoretical-framework.md
  - memory/decisions/agent-23-hypothesis-generator--hypotheses.md
  - memory/discoveries/agent-24-causal-simulator--weakness-register.md
  - memory/discoveries/agent-32-contradiction-analyzer--contradiction-register.md
  - memory/discoveries/agent-33-falsification-agent--counterarguments.md
downstream_consumers:
  - agent-34-confidence-quantifier
  - agent-37-methodology-writer
  - agent-39-discussion-writer
  - agent-42-adversarial-reviewer
---

# Falsification Experiments — Three Killer Tests That Would Kill or Strengthen the Core Claims

## 0. Purpose

This file designs **three empirical experiments** whose results would
**decisively falsify or strengthen** the load-bearing claims of the
manuscript. Each experiment is chosen by three criteria:

1. **High leverage**: its outcome touches at least one headline claim
   (Claim 1, 3, or 4 from the mission brief) and at least one HIGH or
   CRITICAL weakness (W1–W10) or contradiction (IC / EC).
2. **Feasible**: can be executed with resources the author already has
   (Context Graph corpus; Case-3 held-out set; Qwen3-TTS + WavLM
   artefacts; modest compute).
3. **Decisive**: yields a numerical outcome that maps cleanly onto
   accept/reject/qualify decisions for one or more hypotheses.

Each experiment specifies:
- **Hypothesis / claim tested**.
- **Protocol** (pre-registered).
- **Measurable outcome**.
- **Pre-registered kill / strengthen criteria** (what the outcome would
  mean for the claim).
- **Cost estimate** (compute, time, effort).
- **Cross-references** to weaknesses / contradictions / counterarguments
  whose remediation this experiment closes.

Scope discipline: no new primitives (MP-12), no new citations (R3).
The experiments use only artefacts already named in the manuscript's
evidence chain; they are specifications, not new research programmes.

---

## EXP-1 — Cross-encoder SECS triangulation on Case 3

### What it tests

- **H2a (guard efficacy at M=1)**: is the 0.961 mean SECS a
  within-WavLM artefact of the guard, or does it hold under
  architecturally-independent encoder evaluation?
- **H3a (sample-complexity anchor)**: does the $n_m = 50$ operating
  point generalise to a second scorer family?
- **CA-2 partial (multi-modal composition claim degenerates at M=1)**:
  at M=1 the counterargument "this is classical speaker verification"
  is strongest; cross-encoder triangulation is the minimum needed
  defence.
- **W1 CRITICAL remediation**: encoder-matched evaluation on Case 3.
- **IC-3 CRITICAL remediation**: the "per-output verifiable guarantee"
  claim's strongest exposure.
- **EC-10 HIGH remediation**: VALL-E 2 comparison is apples-to-
  oranges partly because of WavLM-base-plus-sv vs. WavLM-large-sv;
  running multiple scorers reveals the encoder-family sensitivity.

### Protocol (pre-registered)

1. **Held-out set**: the exact 10 held-out sentences used in the
   Case-3 report (SITREP 2026-04-16; white paper `alldata.md`).
2. **Guard pipeline**: unchanged — Qwen3-TTS + Full ICL + 50-clip
   centroid + best-of-$K=12$ at $T=0.3$ with $\tau=0.95$ cosine
   threshold vs. WavLM-base-plus-sv centroid (the current guard
   encoder).
3. **Scorers applied to the accepted 10 outputs (post-guard)**:
   - **S1 (incumbent)**: WavLM-base-plus-sv (the guard encoder; baseline
     for sanity).
   - **S2**: ECAPA-TDNN (speaker-verification encoder; widely used in
     the speaker-ID literature; architecturally independent of WavLM).
   - **S3**: Resemblyzer (GE2E / d-vector lineage; independent of
     both WavLM and ECAPA-TDNN).
   - **S4**: WavLM-large-sv (the scorer used by VALL-E 2 in EC-10).
4. **Reference centroid for each scorer**: re-compute the 50-clip
   centroid **in that scorer's embedding space** (L2-normalised mean).
   Each scorer computes its own centroid on the same 50 raw clips; no
   cross-scorer projection.
5. **Metric**: per-sentence SECS (cosine) under each scorer; per-
   scorer mean across 10 held-out sentences; per-sentence accept
   rate at $\tau = 0.95$ in that scorer's space; pairwise Pearson
   correlation across scorers.
6. **Session audit** (W4 remediation): for at least 2 of the 10
   held-out sentences, verify the recording was cross-session with
   the 50-clip reference corpus (different day, ideally different
   mic / room). Report the session relationship.

### Measurable outcomes

- Four scorers × 10 sentences = **40 SECS values**.
- Per-scorer mean SECS $\{\mu_{S1}, \mu_{S2}, \mu_{S3}, \mu_{S4}\}$.
- Per-scorer threshold-pass rate $\{p_{S1}, \ldots, p_{S4}\}$ at
  $\tau = 0.95$ in each space.
- Pairwise scorer correlation $r_{S_i S_j}$.
- Session relationship disclosure table.

### Pre-registered kill / strengthen criteria

**KILL (H2a, H3a, IC-3, EC-10 falsified):**
- $\mu_{S2}$ (ECAPA-TDNN) **< 0.80** on the 10 sentences. Interpretation:
  the 0.961 WavLM number was an artefact of encoder-matched selection,
  and identity-level agreement does not cross encoder families. H2a
  must be reframed from "identity agreement" to "within-WavLM-family
  identity agreement"; the paper cannot invoke the 0.961 as general
  identity evidence.
- $r_{S1, S2}$ **< 0.3** (per-sentence Pearson across the 10). Same
  interpretation: scorer families disagree substantially.

**STRENGTHEN (H2a, H3a confirmed; IC-3, EC-10 resolved):**
- $\mu_{S2} \geq 0.85$ AND $\mu_{S3} \geq 0.85$. Interpretation:
  identity agreement holds across at least three independent scorer
  families; the 0.961 is not an encoder-matched artefact.
- $r_{S1, S2} \geq 0.6$ AND $r_{S1, S3} \geq 0.6$. Scorers agree at
  a per-sentence level, strengthening the identity reading.

**QUALIFY (between kill and strengthen):**
- $0.80 \leq \mu_{S2} < 0.85$ or intermediate correlations. State
  explicitly in §6.3: "Cross-encoder mean SECS of $X$ (ECAPA-TDNN),
  $Y$ (Resemblyzer), indicating partial agreement with the within-
  WavLM measurement; full cross-encoder identity agreement is a
  next-step measurement."

### Cost estimate

- **Compute**: 10 sentences × 4 scorers × re-compute-centroid-from-50-
  clips = negligible (<1 GPU-hour). Scorers are public off-the-shelf
  models.
- **Engineering**: 1–2 days to wire ECAPA-TDNN + Resemblyzer into the
  existing SECS evaluation harness. WavLM-large-sv is a drop-in swap.
- **Total**: ~3 days engineer + trivial compute.

### Leverage

**Highest possible leverage.** Closes W1 (CRITICAL), remediates IC-3
(CRITICAL), softens EC-10 (HIGH), partially blunts CA-2 at M=1.
**This experiment is the single most important test the manuscript
could add before submission.**

### Cross-references

- W1 (agent-24), IC-3 (agent-32), EC-10 (agent-32), CA-2 (agent-33).
- H2a, H3a (agent-23).
- agent-17 Theme D (single-encoder-pair hedge).
- agent-14 Bias-06 (single-encoder).

---

## EXP-2 — Pairwise mutual-information audit of the N=13 Context Graph embedder panel

### What it tests

- **H1 / H1a (signal-count identity at N=13 yields 91 signals/input)**:
  does the $\binom{13}{2}=78$ pairwise term carry non-redundant
  information, or do the embedders collapse substantially to linear
  reparameterisations of each other?
- **Axiom 2 (approximate orthogonality)**: provides the first
  empirical measurement that underpins the axiom, not just the V11
  hedge.
- **CA-1 (91 is loose by one order of magnitude)**: gives an
  empirical lower bound for the effective signal count.
- **W5 HIGH remediation**: unmeasured pairwise MI.
- **H1b falsification hook** (agent-23): "if mutual-information
  measurement reveals effective signal count drops below $n \cdot N$
  … H1b is too weak and the bound needs tightening".
- **RO-8 (agent-19)**: formal MI measurement as named future work;
  this runs a tractable version of it.

### Protocol (pre-registered)

1. **Corpus**: sample **1,000 representative inputs** from the
   production Context Graph corpus, stratified across the 13 embedder
   specialisations (semantic prose, causal text, code, entity-rich
   passages, paraphrase pairs, short queries, long passages, etc.).
   Use real user memories, not synthetic.
2. **Panel**: all 13 production embedders E1 … E13. Compute
   $\phi_m(x)$ for each of the 1,000 inputs and each of the 13
   embedders. For asymmetric embedders (E5, E8, E10), use the
   canonical direction (cause / source / doc) to avoid double-counting.
3. **Dense embedders (E1, E2, E3, E4, E5, E7, E8, E9, E10, E11)**:
   dimensions range from 128 (E12 per-token; not dense in this
   context) to 1536 (E7). For pairwise MI estimation, project each
   embedder's output to a **common 64-dimensional PCA projection**
   learned on the 1,000-input sample (to match scales and avoid
   curse-of-dimensionality in MI estimation).
4. **Sparse embedders (E6, E13)**: convert to fixed-vocabulary sparse
   vectors; report Jaccard and cosine-on-sparse separately; do not
   force into the dense MI framework.
5. **Metric**: pairwise mutual information $I(\phi_j(X); \phi_k(X))$
   estimated via KSG (Kraskov-Stögbauer-Grassberger) on the 64D PCA
   projections, with bootstrapping for error bars. Report in
   **normalised** form: $\mathrm{NMI}_{jk} = 2 I(j; k) / (H(j) + H(k))$.
6. **Redundancy summary**: for each pair, classify as LOW ($\mathrm{NMI}
   < 0.2$), MEDIUM ($0.2 \leq \mathrm{NMI} < 0.5$), HIGH ($\mathrm{NMI}
   \geq 0.5$). Report the histogram over all $\binom{13}{2} = 78$
   pairs.
7. **Effective-signal count** (heuristic): $N_{\text{eff}} = N +
   \sum_{jk} (1 - \mathrm{NMI}_{jk})$. This is a back-of-envelope
   correction to the raw $N + \binom{N}{2}$ count; the proper
   information-theoretic correction is richer but beyond this
   experiment's scope.

### Measurable outcomes

- A 13×13 symmetric NMI matrix.
- The redundancy histogram across 78 pairs.
- $N_{\text{eff}}$ as a single summary number.

### Pre-registered kill / strengthen criteria

**KILL (H1/H1a falsified; CA-1 confirmed):**
- $N_{\text{eff}} < 20$ (i.e., the $C(N,2)$ term contributes less than
  7 effective signals instead of 78). The "91 signals/input" headline
  is exposed as an order-of-magnitude overclaim; H1a must be restated
  as "up to 91; effective count measured at $N_{\text{eff}} = X$, a
  substantial reduction under pairwise redundancy".
- Any embedder pair with $\mathrm{NMI} \geq 0.9$ (near-perfect
  redundancy). Those embedders are not carrying independent axes; the
  panel should be re-counted.

**STRENGTHEN (H1/H1a confirmed; CA-1 blunted):**
- $N_{\text{eff}} \geq 50$ (more than half of the theoretical 91 holds
  empirically). The manuscript can report "up to 91 structured
  signals; $N_{\text{eff}} = X$ under empirical pairwise MI
  measurement", which is strictly stronger than the pure V11 hedge.
- Fewer than 10 of the 78 pairs land in the HIGH-redundancy band.
  Indicates the panel is mostly well-separated.

**QUALIFY (between):**
- $20 \leq N_{\text{eff}} < 50$: state the number directly, treat as
  the empirical value replacing the $N + \binom{N}{2}$ headline.
  Emphasise the **qualitative** DDA story (multiple diverse supervisory
  axes) over the exact count.

### Cost estimate

- **Forward passes**: 1,000 inputs × 13 embedders. Context Graph
  already has these cached for production traffic; cost is
  essentially free if the infra is used.
- **MI estimation**: KSG on 78 pairs × 64D × 1,000 samples. CPU-bound;
  ~1 day on a workstation.
- **Engineering**: 3–5 days to wire the embedder outputs through PCA
  projection + KSG estimation + bootstrap + reporting.
- **Total**: ~1 week engineer + modest compute.

### Leverage

**Very high.** Closes W5 (HIGH), remediates CA-1 (STRONG), promotes
Axiom 2 from "engineering-operational assumption" to "empirically
anchored assumption". Runs a concrete version of RO-8.

### Cross-references

- W5 (agent-24): pairwise MI unmeasured.
- H1, H1a, H1b (agent-23): V11 hedge.
- P1, Axiom 2 (agent-21): approximate orthogonality.
- RO-8 (agent-19): MI future work.
- CA-1 (agent-33): 91 is loose.

---

## EXP-3 — Small-$n_m$ ablation on Case 3 + LoRA-only baseline

### What it tests

- **H3 (sample-complexity $O(M \cdot k)$ bound)**: does the centroid
  remain useful at $n_m = 10, 25, 100$ in addition to the measured 50,
  producing a curve rather than one point?
- **H3a (Case 3 operating point)**: is $k=50$ necessary, or does
  $k < 50$ also suffice? Is $k > 50$ required for replication?
- **H2a (guard efficacy)**: does best-of-$K=12$ at $\tau=0.95$
  alone (no LoRA, or LoRA alone without best-of-$K$) reach 0.961? What
  is TCT's causal contribution?
- **W2 CRITICAL remediation**: missing LoRA-only vs. LoRA+TCT
  ablation.
- **W3 HIGH remediation**: single operating point on $n_m$ curve.
- **CA-3 (STRONG): theorem from one data point.**
- **IC-2 HIGH**: sample-complexity collapse headline vs. one
  empirical point.

### Protocol (pre-registered)

Split into three arms:

**Arm A — $n_m$ sweep (W3 remediation, H3 strengthening).**
1. Four centroid sizes: $n_m \in \{10, 25, 50, 100\}$.
2. Sampled **without replacement** from the same 50-clip source corpus
   (plus 50 additional clips drawn from the same 16-minute capture if
   needed for $n_m = 100$; or alternatively $n_m = 100$ done as
   $n_m = 50$ with a second independent 50-clip draw, documenting the
   sampling).
3. For each $n_m$, re-run the Case 3 pipeline: Qwen3-TTS + Full ICL
   + best-of-$K=12$ at $T=0.3$ + WavLM-base-plus-sv guard at $\tau$
   fitted to the same-session-band floor of that $n_m$ reference set.
4. Evaluate on the same 10 held-out sentences with all four scorers
   from EXP-1.

**Arm B — LoRA-only baseline (W2 remediation, H2a / H3a
attribution).**
1. Run the same 10 held-out sentences through Qwen3-TTS **with the
   Case-3 LoRA applied** but **without** the best-of-$K$ TCT guard
   ($K=1$, no threshold). Report the raw pre-selection SECS
   distribution across 12 samples per sentence (if best-of-$K$ is
   still sampled for the histogram, but no selection is applied).
2. Report the **mean over all 12 samples per sentence** (not just the
   top-$K=1$), both unconditional mean and conditional-on-rejection
   mean.
3. Compare to the post-TCT mean (0.961) to compute the TCT lift.

**Arm C — best-of-$K$ without TCT guard (sampling-diversity control).**
1. Run the LoRA pipeline with $K = 12$ candidates per sentence but
   **no threshold** — select the one with highest SECS to the centroid
   (same as TCT guard at $\tau = 0$, i.e., always accept, choose best).
2. Compare to TCT guard at $\tau = 0.95$ to separate sampling
   diversity from rejection effect.

### Measurable outcomes

- **Arm A**: 4-point curve (mean SECS vs. $n_m$) under each of the 4
  scorers from EXP-1 = 16 values.
- **Arm B**: pre-selection SECS histogram (12 values per sentence × 10
  sentences = 120 data points); unconditional mean; LoRA-only mean
  SECS (single sentence = maximum without threshold = MAX sample).
- **Arm C**: best-of-$K$-without-threshold mean SECS; TCT lift
  quantified.

### Pre-registered kill / strengthen criteria

**KILL Arm A (H3, H3a, W3 falsified):**
- Mean SECS at $n_m = 10$ **< 0.85** or at $n_m = 25$ **< 0.90**:
  Axiom 3's "$n_m \in [10, 10^2]$" claim is falsified at the low end;
  the practical floor for good centroid is $> 25$.
- Non-monotone curve (e.g., $n_m = 25$ outperforms $n_m = 100$):
  indicates the centroid is unstable; Axiom 3 stability claim is
  weakened.

**STRENGTHEN Arm A (H3, H3a confirmed):**
- Mean SECS at $n_m = 10$ **≥ 0.90**: the lower bound of Axiom 3 is
  empirically validated; sample-complexity collapse is stronger than
  measured at $n_m = 50$.
- Monotone non-decreasing or flat curve after $n_m = 25$: indicates
  saturation consistent with Axiom 3's $O(10^2)$ claim.

**KILL Arm B (H2a, H3a attribution falsified; CA-3 confirmed):**
- LoRA-only (no TCT) mean SECS **≥ 0.93**: TCT's marginal contribution
  is **< 0.03 absolute**. The "sample-complexity collapse" is inherited
  from LoRA + 50 clips, not from TCT.  CA-3's "theorem credits TCT for
  what LoRA does" attack is confirmed. §5.4 and §7 attribution must
  change to "TCT verifies; LoRA causes".
- LoRA-only mean SECS **≥ 0.95**: TCT is near-redundant at M=1;
  paradigm value is entirely the multi-modal conjunction (which is
  not yet measured).

**STRENGTHEN Arm B (CA-3 blunted):**
- LoRA-only mean SECS **≤ 0.85** (TCT lift $\geq 0.10$): TCT has a
  clear, large causal contribution. The sample-complexity story
  attributes meaningfully to TCT + DDA beyond what LoRA achieves alone.

**ATTRIBUTION (Arm C):**
- Best-of-$K$-without-threshold mean SECS close to TCT mean: selection
  diversity is doing most of the work; threshold acceptance adds little.
- Best-of-$K$-without-threshold mean SECS **< 0.90** while TCT guard
  gives 0.961: threshold rejection is the decisive mechanism (strong
  validation of TCT Phase 3 design).

### Cost estimate

- **Arm A**: 4 centroid sizes × 10 sentences × $K=12$ = 480 TTS
  generations + re-compute centroids + evaluation × 4 scorers. Qwen3-
  TTS is fast; total is ~1 GPU-day.
- **Arm B**: no new TTS needed if we already store all 12 candidates
  per sentence from the original Case 3 run; if the 12 candidates
  were not retained, it is 120 additional generations (~0.5 GPU-day).
- **Arm C**: same data as Arm B (just a different selection rule).
- **Engineering**: 3–5 days to wire the ablation harness and produce
  the curves.
- **Total**: ~1 week engineer + ~2 GPU-days.

### Leverage

**Critical.** Closes W2 (CRITICAL) + W3 (HIGH) simultaneously.
Arm B is the **single most important ablation** the manuscript can
add for attribution integrity. Arm A converts the empirical
sample-complexity story from "one point" to "a curve".

### Cross-references

- W2, W3 (agent-24).
- H2a, H3, H3a (agent-23).
- IC-2 (agent-32).
- CA-3 (agent-33).
- RO-10 (agent-19): sample-complexity curve.
- Theorem 1 (agent-21).

---

## Summary table

| Experiment | Primary claim tested | Weakness closed | Counterargument closed | Feasibility | Priority |
|---|---|---|---|---|---|
| **EXP-1** Cross-encoder SECS | H2a, H3a (identity agreement) | W1 CRITICAL, W4 MED | CA-2 (at M=1), IC-3 CRITICAL, EC-10 HIGH | ~3 days engineer, trivial GPU | **P0** |
| **EXP-2** Pairwise MI audit | H1, H1a, Axiom 2 | W5 HIGH | CA-1 STRONG, RO-8 future | ~1 week engineer, modest CPU | **P1** |
| **EXP-3** $n_m$ sweep + LoRA-only | H3, H3a, Theorem 1 (empirical) | W2 CRITICAL, W3 HIGH | CA-3 STRONG, IC-2 HIGH | ~1 week engineer, ~2 GPU-days | **P0** |

## Priority order (if only one can be run)

1. **EXP-1** — single highest leverage per cost. Closes the CRITICAL
   encoder-matched exposure. Without it, IC-3 (CRITICAL) and W1
   (CRITICAL) remain open and the paper's headline number is
   vulnerable to a one-sentence review rejection.
2. **EXP-3** — single most important attribution test. Without it,
   CA-3 stands: we cannot causally attribute sample-complexity collapse
   to TCT rather than to LoRA + 50 clips, and the theorem's empirical
   content is one point.
3. **EXP-2** — underlying-axiom measurement. Strengthens Axiom 2
   and blunts CA-1. Without it, the "91 signals/input" headline must
   be restated to "up to 91" with an acknowledged open measurement
   (which is acceptable, but less persuasive).

## What would "kill the paper"?

A single experiment outcome that would force abandoning a load-bearing
claim:

- **EXP-1 KILL** ($\mu_{S2}$ ECAPA-TDNN < 0.80): H2a and H3a lose
  cross-encoder standing. The paper would need to **reframe the
  headline from "0.961 SECS identity match" to "0.961 within-WavLM-
  family agreement"**, and the "per-output verifiable guarantee" claim
  scopes to "within the guard's encoder family". The structural
  framework survives; the empirical headline is demoted.
- **EXP-3 Arm B KILL** (LoRA-only ≥ 0.93): sample-complexity
  collapse attribution shifts from TCT to LoRA + 50 clips. Claim 4
  must be restated: "When best-of-$K$ TCT is layered on a LoRA-
  adapted frozen TTS + 50-clip centroid, the overall pipeline
  achieves $\mu$ SECS 0.961 under within-WavLM-family measurement.
  LoRA alone achieves ≥ $X$; TCT's marginal contribution is $Y$."
  Theorem 1 (constructive) still holds; the causal attribution is
  substantially weaker.
- **EXP-2 KILL** ($N_{\text{eff}} < 20$): the "91 signals/input"
  headline collapses. Restate as "up to 91; measured $N_{\text{eff}}
  = X$". Claim 1 survives qualitatively (multiple independent
  supervisory axes) but loses its quantitative headline.

## What would "dramatically strengthen" the paper?

- **EXP-1 STRENGTHEN** ($\mu_{S2} \geq 0.85$, $\mu_{S3} \geq 0.85$):
  cross-encoder triangulation resolves IC-3 (CRITICAL), removes W1's
  teeth, and repositions the 0.961 as identity-level (not just
  guard-level) agreement.
- **EXP-3 STRENGTHEN** (LoRA-only ≤ 0.85; $n_m = 10$ works):
  TCT's causal contribution is demonstrable; sample-complexity claim
  is anchored in a four-point curve, not one point; Axiom 3 gains
  empirical support at the low end.
- **EXP-2 STRENGTHEN** ($N_{\text{eff}} \geq 50$): the "91 signals/
  input" headline is empirically close to the theoretical bound;
  Axiom 2 is promoted from engineering assumption to measured
  property.

## Recommendations to agent-25 (method-designer) / agent-37
## (methodology-writer)

1. **Bundle EXP-1 and EXP-3 into the Case-3 re-run before manuscript
   submission.** These two experiments remediate four of the five
   highest-severity weaknesses and close the two CRITICAL
   contradictions. The delta in publication quality is substantial.
2. **Run EXP-2 at reduced scope if full is not affordable.** Even a
   back-of-envelope MI estimation on 100 inputs × 13 embedders (not
   1,000) would produce a headline $N_{\text{eff}}$ number that
   dramatically strengthens §3.4.
3. **Pre-register outcomes.** The criteria above should be committed
   before running (kill / strengthen / qualify) to prevent post-hoc
   rationalisation. This aligns with the R4 tier-flagging discipline
   and the T1/T3/T4 evidence tiers of agent-23.

## Changelog

- 2026-04-17 — Initial issue by agent-33-falsification-agent.
