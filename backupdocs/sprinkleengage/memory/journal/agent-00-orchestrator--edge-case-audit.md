---
namespace: journal
agent_id: agent-00-orchestrator
timestamp: 2026-04-19T02:00:00Z
status: active
tags: edge-case-audit, full-state-verification, synthetic-tests
---

# Full-State Verification + Edge-Case Audit — Wave-1

## Source of Truth
- Research docs: `/home/cabdru/contextgraph/docs/sprinkleengage/research/00_target_universe/*.md`
- Memory artefacts: `/home/cabdru/contextgraph/docs/sprinkleengage/memory/{context,decisions,discoveries,handoff,journal,patterns,reports}/*.md`
- Synthesis artefact: `/home/cabdru/contextgraph/docs/sprinkleengage/research/00_target_universe/TARGET_UNIVERSE_RANKING.md`

## Physical Verification (pre-audit)
Directory listing confirmed all claimed files exist on disk:
- 5 research deliverables (4 agents + 1 synthesis)
- 4 final reports
- 4 agent-specific decision files
- 4 handoff files
- 11 discovery files
- 2 context files (mission-brief, constraints)
- 1 pattern file (smap-conventions)
- **Total: 31 files in pipeline.**

File sizes consistent with reported content (43K, 40K, 37K, 28K for research docs; 3-7K for memory files).

---

## Edge Case 1 — "Empty input on primary credential"

### Synthetic scenario
Chris sends the Dohmatob email (Day 0 action, ranking §3) but **omits the HuggingFace LoRA link and the Case 3 MEASURED anchor**. Everything else identical.

### Pre-state (what template says)
Per TARGET_UNIVERSE_RANKING.md §3, the template contains:
> "Measured anchor is Case 3 (§6.3) at 0.961 mean within-WavLM-family SECS on 10 held-out English sentences — encoder-matched scope disclosed. Open-weight LoRA published at huggingface.co/cabdru/shakespeare-lora-gemma4 as a reproducibility anchor."

These two lines ARE the credential stack.

### Expected output (from agent-03 discovery)
Per `memory/discoveries/agent-03-deal-brokers--research-paper-first-pattern.md`:
> "Named-researcher proof anchor dominates acqui-hires. Character (Shazeer/Transformer), Inflection (Suleyman/DeepMind), Adept (Luan/GPT eng) — all three 2024 frontier-lab acqui-hires indexed deal value to named researchers' publication record, NOT product metrics."

And from agent-04 #1 recommendation:
> "Offer: 'We have a paper addressing scope-of-applicability of your model-collapse result to an external-supervision construction (DDA).'"

Without the MEASURED anchor + LoRA, the outreach reduces to "speculative paper with no reproducibility anchor." Response probability drops from **VERY HIGH → LOW-MEDIUM**.

### Post-state test — verification
I ran this check by re-reading agent-03's pattern file and agent-04's top-5-ins decision. Both confirm:
- The LoRA link IS the "reproduction anchor" per Agent 04 Day-7 stock-take logic
- Without it, warm-channel recipients have no low-cost verification path → default to ignore

### Result
**EDGE CASE 1 PASSES** (policy holds): The ranking document already requires both anchor and LoRA in §4 Week-1 plan and §6 Scope Guards. Any email template missing them violates the synthesis.

**Defensive addition**: Adding Blocker below to force Chris to verify both before send.

---

## Edge Case 2 — "Maximum limit: full cluster silence at Day 14"

### Synthetic scenario
No reply by Day 14 from any of: Dohmatob, Khattab, Feng, Kempe, Longpre, Delétang. Zero researcher endorsement.

### Pre-state (what the plan says)
Per TARGET_UNIVERSE_RANKING.md §5 decision logic:
> "ELIF all collapse-cluster + retrieval-cluster silent at Day 14: Pause outreach. Run EXP-2 pairwise-MI audit SELF-FIRST. Publish EXP-2 result as paper v2 update. Re-attempt cold outreach with Neff measured (replaces 'up to 91 signals' with 'Neff = ??'). Confidence: LOW on 9-month $3B path; HIGH on 18-month path with strengthened paper."

### Expected output
Fallback exists, is technical (self-runnable), and does not require external gate. Chris can independently execute EXP-2 since he owns the 13-embedder Context Graph substrate.

### Post-state test — verification
I check against paper §9:
- Paper §9 names three open experiments: EXP-1 (ECAPA re-score, external data dependency), EXP-2 (pairwise MI, self-runnable on Context Graph panel), EXP-3 (nm sweep, self-runnable on Case 3 audio).
- EXP-2 is genuinely self-runnable: 100-1000 representative inputs × 13 embedders × MINE or KSG estimator. Chris's substrate has the inputs and the embedders.

Cross-checked with Agent 04 handoff:
> "If all collapse-cluster + retrieval-cluster silent at Day 14 → Pause outreach. Run EXP-2 pairwise-MI audit SELF-FIRST."

Consistent.

### Result
**EDGE CASE 2 PASSES**: fallback is documented, technically executable, and preserves optionality. Timeline extends 3-6 months but the outcome-tree survives.

---

## Edge Case 3 — "Invalid format: scope violation in outreach"

### Synthetic scenario
Chris sends the Qatalyst cold email but writes: *"Our voice cloning achieves 0.961 speaker verification — state of the art by a wide margin."*

This OMITS "within-WavLM-family" and OMITS "N=10, one speaker, English" and OMITS the ECAPA-TDNN triangulation caveat. It also asserts "state of the art" (comparative claim not defended in paper).

### Pre-state
Per TARGET_UNIVERSE_RANKING.md §6 Scope Guards (mirrored from paper §8.L1):
> "Case 3: MEASURED, encoder-matched (WavLM-base-plus-sv guard + evaluator), N=10 held-out English sentences, one speaker, one session. ECAPA-TDNN cross-encoder triangulation (EXP-1) pending."

And from `memory/context/constraints.md`:
> "**Case 3 scope**: encoder-matched (WavLM-base-plus-sv guard + WavLM-base-plus-sv evaluator), N=10 held-out English sentences, one speaker, single session, single encoder family. **Never claim cross-encoder identity verification** without ECAPA-TDNN re-score (EXP-1 pending)."

The constraint is explicit: "Never claim cross-encoder identity verification."

### Expected output
If this email reaches Dohmatob (collapse-cluster reviewer), he immediately catches:
1. "State of the art" is a benchmark-contest claim the paper §7.3 explicitly rejects.
2. Omitted scope → either ignorance (bad) or dishonesty (worse).
3. Missing ECAPA-TDNN caveat → he knows EXP-1 is in paper §9 open-problems register.

Reputation damage is categorical: **trust broken permanently**. Collapse cluster does not re-engage.

### Post-state test — verification
I scan all four agent deliverables + constraints + ranking for any occurrence of:
- "state of the art" applied to Teleox: NOT FOUND (good — no violation in package).
- "0.961" without WavLM-family qualifier: NOT FOUND (good — every occurrence is properly scoped).
- "cross-encoder verified": NOT FOUND (good).

Checked agent-01 frontier-labs handoff, agent-02 sovereign handoff, agent-03 broker handoff, agent-04 warm-channels handoff, ranking §3/§4/§6. All preserve scope language.

### Result
**EDGE CASE 3 PASSES** package-side: every template in the ranking includes scope-preserving language. But this is a **live-fire risk** — Chris must personally preserve scope when composing actual emails. Adding Blocker to enforce this.

---

## Blockers Added (preventive)

1. **Pre-send checklist mandatory for every outreach** — written to `./memory/blockers/agent-00-orchestrator--pre-send-checklist.md`:
   - [ ] Email contains MEASURED anchor (Case 1 or Case 3) with scope tag
   - [ ] Email contains LoRA HF link (reproducibility anchor)
   - [ ] Email contains CI disclosure line
   - [ ] Email does NOT contain "state of the art", "beats RLHF", "refutes collapse", "cross-encoder verified"
   - [ ] Email contains paper section citations (§ symbols) for technical claims

2. **Day 14 fallback trigger** — written to `./memory/blockers/agent-00-orchestrator--day14-fallback.md`:
   - If all cluster silent at Day 14, HALT commercial outreach.
   - Execute EXP-2 pairwise-MI audit on Context Graph's 13-embedder panel (self-runnable).
   - Publish result as paper v2.

---

## Conclusion

All 3 edge cases pass at the package level. The wave-1 output is internally consistent, scope-preserving, and has documented fallbacks. The live-fire risk is in email composition, not in the synthesis; the two blockers above are the mitigation.

## Competing Interests
Chris Royse is founder/operator of Teleox.ai (primary) and Leapable.ai (companion MCP marketplace).
