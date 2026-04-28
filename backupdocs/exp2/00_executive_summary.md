# EXP-2 Executive Summary

**One page. For Chris, Steve, and any counsel who needs to understand the experiment in five minutes.**

---

## The question EXP-2 answers

Does the 13-embedder Context Graph panel actually produce $\approx 91$ independent supervisory signals per input, or are the embedders so correlated that the effective count collapses to a number in the single digits?

Formally: for every pair $(\phi_j, \phi_k)$ of embedders in E1–E13, estimate
$$I(\phi_j(X);\, \phi_k(X)) = H(\phi_j(X)) - H(\phi_j(X)\,|\,\phi_k(X))$$
over a sample $X \sim \mathcal{P}_\text{production}$ of real inputs, and aggregate the 78 pair estimates into a single number $N_\text{eff}$ that represents the **measured effective count of unique supervisory signals**.

---

## Why we have to run this ourselves, in the next 21 days

1. **We cited "up to 91" in the NeurIPS-submitted position paper.** Axiom 2 (approximate embedder independence) is the load-bearing premise for that number. Axiom 2 is contested by Huh et al. 2024 (*Platonic Representation Hypothesis*, ICML 2024), which empirically shows modern SSL encoders converge on near-shared representations. Any reviewer or diligence team can run this test in ~72 hours.

2. **The panel contains a known redundancy we have not quantified.** E1 (Semantic, e5-large-v2) and E8 (Graph, e5-large-v2) share the same backbone weights. They differ only in a learned source/target linear projection. This pair is the EXP-2 "near-linear reparameterization" case and serves as both sanity check and upper-bound redundancy estimator.

3. **Asymmetric deal consequence.** If we run it first:
   - **Strong** ($N_\text{eff}$ 70–91): we own the headline; ceiling confirmed.
   - **Moderate** (25–50): we frame the scope from "91 signals per input" to "order-of-magnitude supervisory signal multiplication" and keep pricing power.
   - **Weak** (8–15): we softened the pitch before anyone else measured; reframing to three-role composition + extensibility holds.

   If a lab runs it first and finds a weak result, our price gets halved in the same meeting.

---

## What counts as success

**Success of the experiment is not a specific $N_\text{eff}$ number.** Success is:
1. Pre-registered protocol — all hyperparameters, estimators, sample sizes, decision rules locked **before** the first MI is computed (freeze 2026-04-29).
2. Three independent estimators (MINE / InfoNCE / KSG) on the same panel and sample. Disagreement reported, not hidden.
3. Bootstrap 95 % CIs on every pair estimate. Point estimates alone are not publishable.
4. Controls: a synthetic "copy of E1" embedder ($\phi_\text{copy} = \phi_{E1}$) must yield $I \approx H(\phi_{E1})$; two independently seeded random-projection encoders must yield $I \approx 0$. Both controls pass before any real-pair estimate is trusted.
5. Results written in **constructive-ceiling-plus-realized-floor** form: 91 stays in the paper as a structural bound; $N_\text{eff}$ is the operating number.

---

## Cost and timeline

- **Compute**: single RTX 5090 node we already run production on. One full pass (1000-input corpus, 78 pairs, 3 estimators, 5 seeds, 1000 bootstrap samples) fits in ~36 GPU-hours. Incremental passes on demand.
- **Calendar**: kickoff 2026-04-30 (Day 7 of the 14-day list); results 2026-05-14 (Day 21); first external surface in Perez memo revision Day 22+ if moderate/strong, or in reframed memo if weak.
- **Labor**: Chris implementation (80 %), Steve compliance review on the reporting language before any external surface (20 %).
- **Rollback**: if estimators disagree by > 2 bits on > 20 % of pairs, halt and re-examine before publishing. See [10_risk_register.md](10_risk_register.md).

---

## What EXP-2 does NOT answer

- **Task-relevant MI.** $I(\phi_j; \phi_k)$ measures embedder-to-embedder redundancy, not embedder-to-task information. A pair can have high $I$ and still be jointly useful if each captures different task-relevant subspaces. [14_followup_experiments.md](14_followup_experiments.md) covers the conditional-MI and task-labeled follow-up.
- **Causal composition validity.** The counting identity involves pairs *(retriever, labeller)* composed through G_τ. EXP-2 measures the raw substrate, not the three-role composition. That is a separate experiment.
- **Cross-domain generalization.** EXP-2 runs on the production input distribution. Claims about effective-N on OpenWebText, RedPajama, or code-only corpora require separate passes.

---

## Single decision rule for Chris

**If EXP-2 completes on time with estimator agreement and bootstrap CIs tighter than 1 bit**, update the Perez memo and any patent disclosures with the measured $N_\text{eff}$ alongside the constructive 91 ceiling, and proceed with the 14-day action list unchanged.

**If it does not**, send the Perez memo Day 10 with the constructive ceiling only, footnote "realized floor pending EXP-2, results in 2 weeks," and do not delay patent filing (Day 14). Per the action list: do not wait on EXP-2 to send the memo.

---

*Full protocol: [01_preregistration.md](01_preregistration.md). Public-facing framing: [12_publication_strategy.md](12_publication_strategy.md).*
