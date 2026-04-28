# EXP-2 Failure-Mode Playbook

**What to say / do for each of the three outcomes (Strong / Moderate / Weak).** Pre-commits Chris's response before the data arrives so the decision isn't emotional.

---

## 1 · Strong ($N_\text{eff,pair} \ge 70$ with CI lower bound $\ge 70$)

### 1.1 What it means technically
Axiom 2 approximately holds on the production distribution. Pair redundancy is either small in absolute terms or concentrated in known shared-backbone pairs (E1–E8, E6–E13). The measured 77 % of constructive ceiling is a cleaner-than-expected result.

### 1.2 Prior probability
10 %. Unlikely given E1-E8 shared backbone alone contributes ≥ 5 bits of redundancy. If we get Strong, it is because the panel's cross-family embedders (E5, E7, E9, E11) reach the E1-E8 ceiling level of uniqueness on this distribution.

### 1.3 Headline for Perez memo
See [12_publication_strategy.md §2](12_publication_strategy.md) — "Strong outcome" template.

### 1.4 Commercial playbook move
1. **Perez revision Day 22+** with the number + reproducibility link.
2. **No pitch change**. The original framing ("up to 91 with Axiom 2 supported") proceeds.
3. **Pricing power preserved**. Anchor values in [PROBABILITY_RECALIBRATION.md](../../teleoxwebsite/docs/gametheoryinvestigation/executive/PROBABILITY_RECALIBRATION.md) hold: Anthropic $4.0B, Google $6.0B.
4. **Marketing**: blog post + HF card updates with clean numbers. Strong signal to the arXiv/USPTO-monitored ecosystem that we measure our claims.

### 1.5 What to watch for
- Steve flags if the language drifts to "proves" or "solves" — NEVER use those words even with a Strong result.
- Confirm at least one non-trivial high-MI pair in the 78 — otherwise the result reads as "contrived low-redundancy panel."
- Expect a lab to replicate; our result should be the first citation in the replication paper.

### 1.6 Counterfactual if a lab runs EXP-2 first and finds Moderate or Weak
Minor asymmetry: even if their panel is differently composed (and differently constructed from the one we pre-registered), the cited protocol differences explain the gap. Tone: collegial, not defensive. "Our panel + their panel + protocol differences" is the most credible story.

---

## 2 · Moderate ($25 \le N_\text{eff,pair} < 70$; CI lower bound $\ge 25$)

### 2.1 What it means technically
Panel has substantial redundancy (expected) but also substantial unique signal. The 91 constructive count is not realized; the $\hat{N}_\text{eff}$ value (typically in 30–50 range) is the effective signal count.

### 2.2 Prior probability
55 %. This is the modal outcome. Most likely:
- E1-E8-E10 cluster shares ~6–8 bits pairwise
- E6-E13 twin shares ~4–6 bits
- E2-E3-E4 triple shares ~5 bits
- Cross-family pairs contribute 1–3 bits each
- Net: $N_\text{eff}$ in 28–42 range

### 2.3 Headline for Perez memo
See [12_publication_strategy.md §2](12_publication_strategy.md) — "Moderate outcome" template.

### 2.4 Commercial playbook move
1. **Perez revision Day 22+**. Frame as honest audit, not a retreat.
2. **Pitch reframes** from "up to 91 signals" to "measured $\hat{N}_\text{eff}$-fold multiplication + $O(N)$ growth." Extensibility is the load-bearing claim, not the absolute count.
3. **Anchor values hold**. Moderate is commercially defensible at the playbook's stated anchors; the pitch credibility actually increases from the transparency.
4. **New attention** on the three-role composition + $G_\tau$ predicate as independent primitives from the counting identity. These were always in the stack; the emphasis shifts toward them.

### 2.5 What to watch for
- Steve flags if any line reads as a retreat. "Moderate" is not "weak"; the language should be confident: "measured multiplicative advantage" not "failed to achieve ceiling."
- Ensure the per-pair table leads with the cross-family pairs (showing real unique signal), not the shared-backbone pairs (showing expected redundancy).
- The growth-rate table ([06_analysis_and_decision_rules.md §8.4](06_analysis_and_decision_rules.md)) is the key exhibit — shows that adding embedder 14 still adds $O(N)$ pair signals.

### 2.6 Reframed Perez paragraph (preapproved)
> The audit finds effective $\hat{N}_\text{eff} = X$ on the production distribution versus the 91 constructive ceiling. The multiplicative advantage over single-encoder baseline ($\hat{N}_\text{eff} \times$) is the operating number; each new embedder still contributes $O(N)$ new pair signals. This is the realized floor of a supervisory-density primitive that has measured substrate value independently of specific model choices. The pre-registered protocol, MINE + InfoNCE + KSG estimator triple, and bootstrap CIs are attached.

### 2.7 Follow-up experiment priority
Conditional MI ([14_followup_experiments.md §2](14_followup_experiments.md)) — does a task-informative label $Y$ reduce the measured redundancy? If $I(\phi_j; \phi_k | Y)$ is much smaller than $I(\phi_j; \phi_k)$, then much of the observed pair MI is on non-task dimensions. This is a strong follow-up card to play in Week 3–6 of the Perez engagement.

---

## 3 · Weak ($N_\text{eff,pair} < 25$; CI lower bound < 25)

### 3.1 What it means technically
Huh et al. 2024's PRH prediction holds on the production panel. Cross-family embedders have substantially more shared information than expected. The 91 constructive count is a ceiling, not a floor.

### 3.2 Prior probability
35 %. Not modal but not improbable. Likely sub-scenarios:
- **Weak-mild** ($N_\text{eff} \in [8, 15]$): expected under strict PRH. Reframing proceeds per below.
- **Weak-catastrophic** ($N_\text{eff} < 8$): triggers R-13; commercial playbook pivots mid-flight.

### 3.3 Headline for Perez memo
See [12_publication_strategy.md §2](12_publication_strategy.md) — "Weak outcome" template.

### 3.4 Commercial playbook move

**First 48 hours after result**:
1. Steve gate on every utterance; Chris does not speak externally until the reframed pitch is drafted.
2. Convene Chris + Steve + counsel for framing sign-off.
3. Write the reframed Perez memo. Do not send until Day 28 at earliest.

**Weeks 1–2 of reframing**:
1. **Shift the headline**. The counting identity is preserved as a ceiling; the realized floor is lower. But the commercial substrate value sits elsewhere:
   - Three-role composition (retriever, labeller, $G_\tau$) is a distinct primitive
   - Per-output $G_\tau$ cosine predicate is a separate measurable outcome
   - $O(N)$ growth rate with embedder count remains valid
2. **Lead with Case 1 (Shakespeare LoRA)** in the Perez memo instead of the counting identity. The Shakespeare result is the working example of the three-role composition producing real downstream capability.
3. **Drop the "91 signals" framing from all public-facing language** until the reframed pitch is stable.
4. **Accept lowered deal anchors** per WINNING_PLAYBOOK PATH-4 / PATH-5 probability shifts. Anthropic anchor shifts toward $2.0B; Google toward $3.0B.

**Day 28+ Perez re-engagement**:
> The pairwise MI audit returned effective $\hat{N}_\text{eff} = X$, substantially below the constructive 91 ceiling. This is consistent with the Platonic Representation Hypothesis literature (Huh et al. 2024) and underscores that the substrate's commercial value is not embedding-panel-wide independence but the **compositional structure** over whatever substrate exists: the three-role retriever / labeller / $G_\tau$ per-output predicate, with the $O(N)$ growth rate preserved for future panel additions. Measurement was under pre-registered protocol; reproducibility attached. The result strengthens rather than weakens the case that operator-discipline over a well-calibrated substrate is the durable DDA primitive — embedding panel composition matters less than the three-role pipeline that operates over it.

### 3.5 What to watch for (weak-mild)
- Halt any pitch using "91" as the headline. 91 stays in the paper as a ceiling but loses prominence.
- Do not litigate the result. The PRH literature predicts it; the measurement confirms it. Publishing honestly maintains scientific credibility.
- Pricing power drops but does not collapse: moderate deal anchors hold if the compositional pivot is clean.

### 3.6 What to watch for (weak-catastrophic, $N_\text{eff} < 8$)

**This triggers R-13**. Actions:
1. **Halt ACTION #16** (Stebbings pitch) — do not approach podcast with weak headline.
2. **Halt ACTION #20** (dean-advisor) — LeCun and Hutter will see the result before their introductions hit; better to have a reframed pitch first.
3. **Pause ACTION #18** (bridge round) — the FF bridge round materials cite the 91 identity; those need revision before the round opens.
4. **Accelerate ACTION #17** (banker preliminary). A banker with a re-framed pitch (three-role composition, $G_\tau$, Case 1) is the recovery path; do not delay banker engagement on a bad-result embarrassment window.
5. **Freeze patent claim revisions.** The provisional filed Day 14 already stands; the non-provisional window (12 months) is when the claim scope is re-negotiated based on the realized floor.

### 3.7 Reframed pitch primitives (Weak scenario)

Three claims that DO NOT depend on Axiom 2:

1. **Three-role composition**: retriever (any embedder panel) + labeller (LLM with grammar-constrained output) + $G_\tau$ per-output cosine predicate is a structural primitive. Not embedding-panel-independence, but the composition itself.
2. **$G_\tau$ deterministic predicate**: per-output cosine-similarity thresholding is a measurable, testable property class independent of the substrate below it. (Shumailov scope-ladder applies; different property from recursion collapse.)
3. **$O(N)$ growth curve**: even at $N_\text{eff} = 8$, each new embedder added to the panel contributes on the order of $N$ new pair signals. The growth rate is preserved; the absolute count is not.

These three primitives are the fallback pitch. They were always in the stack; they become load-bearing if EXP-2 is weak.

### 3.8 Language to AVOID in Weak reframing
- "The counting identity was wrong" — it wasn't; it was a constructive upper bound
- "We were over-optimistic" — we were not; we committed to publishing the realized floor
- "Axiom 2 is falsified" — Axiom 2 was stated as an approximation; the audit quantifies it
- "The 91 number is meaningless" — it isn't; it's a structural invariant
- "We need to pivot" — call it a "refocus" or "re-emphasis"; "pivot" implies the previous direction was wrong

### 3.9 Language TO USE in Weak reframing
- "Constructive ceiling 91; realized floor $\hat{N}_\text{eff} = X$"
- "The three-role composition primitive is independent of pair independence"
- "$O(N)$ growth curve is preserved"
- "PRH-consistent on learned SSL encoders"
- "The audit is the methodological contribution regardless of the numerical outcome"

---

## 4 · Decision tree (one-page summary)

```
                    EXP-2 result
                          |
         +----------------+----------------+
         |                |                |
      Strong           Moderate          Weak
   (P(Strong)=0.10)  (P(Moderate)=0.55) (P(Weak)=0.35)
         |                |                |
   Perez Day 22      Perez Day 22      Halt outreach;
   with headline     with reframed     reframe Perez
   per §1.3          headline          Day 28+ per §3
         |           per §2.3
         |                |                |
   No pitch change    Extensibility    Three-role +
   Anchors hold       = headline       G_tau + O(N)
                      Anchors hold     = headline
                                       Anchors drop 30%
         |                |                |
   Blog + HF update  Blog + HF       No public comms
   immediate         after ≥1 day    for 7-14 days
                     Steve review    until reframed
                     for tone        pitch stable
```

---

## 5 · Chris's commitment in advance

- **I will not argue with the data.** Whatever EXP-2 returns is what the measurement is.
- **I will not re-run until I get a better number.** One pre-registered run is the record.
- **I will not split the result across audiences** (tell Perez one number, tell the public another).
- **I will follow the playbook for the outcome that arrives**, even if my gut says otherwise.
- **I will disclose the $N_\text{eff}$ number to every diligence party** after the patent lock, regardless of outcome.
- **I will preserve the reproducibility artifacts** (seeds, PCA, sketch matrices, critic state hashes) so any party can reproduce the measurement bit-for-bit.

Signed: Chris Royse, Steve Abbey. (Electronic sign-off at preregistration freeze 2026-04-29.)
