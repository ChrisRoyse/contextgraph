---
namespace: handoff
agent_id: agent-34-confidence-quantifier
timestamp: 2026-04-17T09:00:00Z
status: active
tags: handoff, agent-35, introduction-writer, confidences-ready
---

# Handoff: Agent-34 (confidence-quantifier) → Agent-35 (introduction-writer)

## Read these files FIRST, in this order

1. `./memory/patterns/agent-34-confidence-quantifier--per-claim-confidence.md` —
   the calibrated per-claim confidence table (C1 … C14) plus paper-
   level scenarios (§3) and reviewer-persona roll-up (§5).
2. `./memory/patterns/agent-34-confidence-quantifier--confidence-language-for-writers.md` —
   paste-ready hedges and the banned-phrase list.
3. `./memory/patterns/agent-33-falsification-agent--claim-softening-recommendations.md` —
   six paste-ready language edits (2 MUST, 3 SHOULD, 1 MAY).
4. `./memory/reports/agent-34-confidence-quantifier--final-report.md` —
   SMAP summary.

## What agent-34 did (summary)

- Calibrated 14 claims (C1 … C14) against agent-23's locked
  confidence priors, applying pressure from agent-24 (W1–W10),
  agent-29 (VT-* register), agent-32 (IC-* / EC-*), and agent-33
  (CA-1 … CA-6) counterargument verdicts.
- Produced post-softening Open-Philanthropy-style confidence levels
  assuming the two MUST-softenings (CA-1 "up to 91"; CA-3 constructive
  vs. empirical split) are applied in the draft.
- Derived paper-level submittability confidence: **~78% baseline**
  under current draft + softenings; bands up to **~93%** if EXP-1
  + EXP-3 both strengthen; bands as low as **~40%** if EXP-1 kills.
- Produced writer-facing language cue-sheet with paste-ready
  sentences, per-section checklists, and a banned-phrase table.

## Key findings for agent-35 (introduction-writer)

### Per-claim confidence snapshot (post-softening)

| Claim | Confidence | Language band |
|---|---|---|
| C1 up-to-91 signals/input | **≥95%** | "virtually certain" |
| C2 fourth compression category | **~85%** | "highly confident" |
| C3 per-output verifiability (panel-relative) | **~90%** | "highly confident" |
| C4 Shumailov-exempt by construction | **~85%** | "highly confident" |
| C5 0.961 WavLM SECS (encoder-matched) | **≥98%** | "virtually certain" (scoped) |
| C6 DNSMOS-P.808 parity | **~85%** | "highly confident" |
| C7 T=0.3 optimal (this config) | **~75%** | "confident" |
| C8 50-clip > 1-clip by +0.015 | **~85%** | "highly confident" |
| C9 Guard ON/OFF on Case 2 | **~50%** | "roughly even odds" |
| C10 Mouth-weighted loss necessary | **~55%** | "roughly even odds (lean yes)" |
| C11 Case 1 retrain 15/15 FSV | **~60%** | "roughly even odds (lean yes)" |
| C12a Sample-complexity theorem (constructive) | **≥95%** | "virtually certain" |
| C12b Sample-complexity empirical | **~40%** | "leaning against (strong phrasing)" |
| C13 TCT vs scalar-reward (pool-relative) | **~80%** | "highly confident" |
| C14 Research not marketing | **~80%** | "highly confident (CONDITIONAL on FM-01)" |

### Paper-level confidence

**~78% confident the paper is arXiv-submittable** under the current
draft plus PENDING remediations (softenings applied; no new
experiments yet).

| Scenario | Paper submittability |
|---|---|
| Softenings NOT applied | ~55% |
| Softenings applied, no new exps (**baseline**) | **~78%** |
| + EXP-1 STRENGTHEN | ~88% |
| + EXP-3 Arm B STRENGTHEN | ~88% |
| + EXP-1 and EXP-3 STRENGTHEN | ~93% |
| + EXP-1 KILLS | ~40% |
| + FM-01 regex FAILS | ~55% |

### Lowest-confidence claims (MUST be softened most in §1)

1. **C12b (sample-complexity empirical across modalities, ~40%)** —
   agent-35 MUST use the constructive/empirical split in the §1
   headline. Never claim empirical collapse "across modalities"
   without qualifying "anchored at one T1 operating point".
2. **C9 (Case 2 guard effect, ~50%)** — if §1 alludes to Case 2
   results, prefix with "**projected (T3)**".
3. **C11 (Case 1 retrain, ~60%)** — if §1 alludes to Case 1,
   "SFT+DPO retrain in progress at time of writing".
4. **C10 (mouth-weighted necessity, ~55%)** — use "consistent with",
   never "necessary".
5. **C7 (T=0.3 optimal, ~75%)** — always scoped to this pipeline.

### Highest-confidence claims (§1 may state strongly)

- **C1** "up to 91 signals/input" — strong structural upper bound.
- **C5** "0.961 WavLM SECS under encoder-matched protocol" with
  mandatory W1 disclosure sentence.
- **C12a** constructive sample-complexity theorem.
- **C3** per-output verifiability relative to the panel.

## What agent-35 (introduction-writer) must do

### Priority 1 — Headline calibration

- **Opening paragraph**: frame DDA as "a third path around the data
  wall" (not "solves the data wall"). Use C1 "up to 91 signals/input"
  language exactly.
- **Case-3 one-line mention (if used)**: "0.961 mean WavLM SECS on
  10 unseen sentences under an encoder-matched protocol (§6.3; see
  W1 scope)". Do NOT drop the encoder-matched modifier.
- **Sample-complexity claim in §1**: use the split framing
  verbatim — "constructive theorem + one T1 operating point".

### Priority 2 — Contribution list calibration

Each contribution bullet must match its calibrated confidence:

- "We propose …" (✓ for any theoretical contribution — low-stakes
  hedging verb).
- "We derive …" (✓ for C1, C12a — derivations under axioms).
- "We measure …" (✓ for C5–C8 with encoder-matched scope).
- "We demonstrate …" (**only** for C5 encoder-matched scope; **never**
  for C12b across-modalities).
- "We show …" (caution — use only for derivations or for C5).

### Priority 3 — §1 must forward-reference §8 Limitations

The introduction should include a short "scope & limitations preview"
paragraph that names:
- Encoder-matched evaluation on Case 3 (W1 / IC-3);
- Pre-selection distribution / LoRA-only attribution (W2 / IC-2);
- Single $n_m$ operating point (W3);
- No $M>1$ empirical on Case 2 (W7).

### Priority 4 — §1 must acknowledge CI / FM-01

The introduction or Ethics pointer must include a one-sentence
commercial-interest acknowledgement to carry C14's conditional.

## Open carries agent-34 passes to agent-35 and downstream

1. **Banned-phrase list** is load-bearing. Agent-42 will regex-check
   (FM-01) for offending phrases.
2. **Every empirical claim needs T1 / T3 tagging** (measured /
   projected) somewhere close to its statement.
3. **Every "up to" / "pool-relative" / "within-WavLM-family" modifier
   is non-negotiable** — dropping it restores the claim to its
   unsoftened (and indefensible) form.
4. **EXP-1, EXP-2, EXP-3 do not change §1 as submitted now** (they
   are agent-25 remediations pending execution). If they land before
   submission, agent-34 will re-calibrate and agent-35 updates.

## Files agent-34 produced

- `./memory/patterns/agent-34-confidence-quantifier--per-claim-confidence.md`
- `./memory/patterns/agent-34-confidence-quantifier--confidence-language-for-writers.md`
- `./memory/handoff/agent-34-to-35--confidences-ready.md` (this file)
- `./memory/reports/agent-34-confidence-quantifier--final-report.md`

## Carried hedges from upstream (unchanged)

- agent-01 V11, V18, V22, R3, R4; FM-01, FM-04.
- agent-14 Bias-05, Bias-06, Bias-08, Bias-10.
- agent-17 Themes 2, 3, 5, 7; Register R1, R3, R4.
- agent-21 axioms 1–3 hedges; P1–P5 confidence; T1 / T2 pool-relative.
- agent-23 §7 evidence tiers; §8 falsification precedence.
- agent-24 W1 CRITICAL; W2 CRITICAL; W3, W5, W7 HIGH.
- agent-29 VT-I1, VT-I2, VT-I3, VT-C1, VT-S4.
- agent-31 §Ethics (consent, dual-use, asymmetric release); §CI.
- agent-32 IC-1 … IC-6; EC-7 … EC-11.
- agent-33 CA-1 … CA-6 counterarguments; softening recs 1–6.

## One-paragraph summary for agent-35

> Agent-34 calibrated 14 load-bearing claims against agent-23's
> priors and agent-33's adversarial pressure. Under the assumption
> that agent-33's two MUST-softenings are applied (CA-1 "up to 91";
> CA-3 constructive/empirical split), the paper's per-claim
> confidence is **≥95%** on the theoretical spine (C1, C3, C4, C12a),
> **~85–98%** on the encoder-matched Case-3 empirical spine (C5–C8),
> **~40–65%** on the projected claims (C9–C11, C12b), and **~80%** on
> pool-relative / commercial-framing claims (C13, C14). Paper-level
> submittability confidence is **~78%** under current draft plus
> softenings, rising to **~93%** if EXP-1 (cross-encoder SECS) and
> EXP-3 (TCT-delta / $n_m$ sweep) both STRENGTHEN, and dropping to
> **~40%** if EXP-1 KILLS. Agent-35 must use the writer cue-sheet's
> paste-ready hedges — in particular, never strip "up to", "under
> the encoder-matched protocol", "constructive theorem + one T1
> anchor", and "pool-relative" modifiers from claims that carry them.

## Quality-gate status delivered by agent-34

| Gate | Status |
|---|---|
| Per-claim confidence assigned for all 14 claims (C1–C14) | **PASS** |
| Each confidence has explicit justification | **PASS** |
| Paper-level confidence with scenario bands | **PASS** |
| Writer-facing language cue-sheet with banned-phrase list | **PASS** |
| Consistent with agent-33's recommended priors (within ±5 pp) | **PASS** |
| Softening assumptions stated explicitly | **PASS** |
| Pre- vs. post-experiment bands for EXP-1 / EXP-2 / EXP-3 | **PASS** |
| Reviewer-persona roll-up (optional, Priority 3) | **PASS** |
| No new primitives introduced | **PASS** (MP-12) |
| No new citations introduced | **PASS** (R3) |
| Handoff coherence to agent-35 | **PASS** |

## Confidence

**HIGH** that the 14 claims cover the paper's load-bearing assertions
without omission.

**HIGH** that the calibrated confidence levels are consistent with
agent-33's red-team baseline (max deviation ±5 pp).

**HIGH** that the writer cue-sheet's paste-ready hedges will prevent
the top review-kill vectors (CA-1, CA-3, CA-6) if followed.

**MEDIUM** on the exact scenario-band values in §3 of the per-claim
table; these depend on EXP-1 / EXP-3 noise assumptions that are
indicative rather than measured.

**HIGH** that agent-35 has all the material needed to write a
calibrated introduction paragraph and contribution list.

## Changelog

- 2026-04-17 — Initial handoff by agent-34-confidence-quantifier.
