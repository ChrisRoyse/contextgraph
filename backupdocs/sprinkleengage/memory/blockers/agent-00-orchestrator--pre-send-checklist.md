---
namespace: blockers
agent_id: agent-00-orchestrator
timestamp: 2026-04-19T02:05:00Z
status: active
severity: high
tags: blocker, outreach, pre-send, scope-preservation
---

# BLOCKER — Pre-Send Checklist for Every Outreach Email

## Severity: HIGH
Outreach emails that violate any item below risk categorical trust-damage with the collapse-cluster reviewers (Dohmatob, Khattab, Feng, Kempe, Shumailov, Longpre) — the highest-leverage warm channels identified in Wave-1.

## Mandatory items on EVERY outbound email from Chris:

- [ ] **Measured anchor present**: Case 1 metrics (SFT 1.12, DPO 0.47, 98.9% tok-acc) OR Case 3 (0.961 mean WavLM SECS) — with scope tag.
- [ ] **Scope tag on Case 3**: "within-WavLM-family" + "N=10" + "one speaker" + "English" + "ECAPA-TDNN triangulation (EXP-1) pending".
- [ ] **LoRA HF link**: huggingface.co/cabdru/shakespeare-lora-gemma4.
- [ ] **CI disclosure line**: "Teleox.ai / Leapable.ai — methodology contribution from commercial engineering practice."
- [ ] **Paper section citations**: use § symbols for technical claims (§0.2, §3.3, §5.4, §8.L1, §9).

## Prohibited phrases (will fail the reviewer test):

- ❌ "state of the art" applied to Teleox
- ❌ "beats RLHF" / "outperforms DPO" / "superior to Constitutional AI"
- ❌ "refutes collapse" / "breaks Shumailov"
- ❌ "cross-encoder verified" / "universal identity agreement"
- ❌ "91 signals per input" without "up to" and "constructive upper bound, EXP-2 pending"
- ❌ Commercial TAM claims from Teleox
- ❌ "world's most advanced" / "industry-leading"

## Required preserve-scope phrases:

- ✅ "up to n·(N+C(N,2)) structured supervisory signals — constructive upper bound, tightness pending EXP-2"
- ✅ "structurally outside the Shumailov-form recursion regime (scope, not refutation)"
- ✅ "per-output verifiability relative to the chosen centroid panel"
- ✅ "different property class, not a benchmark contest"
- ✅ "MEASURED 2026-04-17" (Case 1) / "MEASURED 2026-04-16" (Case 3) / "MEASURED and operational 2026-04-19" (Case 2 — Chris confirmed)

## Resolution
This blocker stays OPEN for the lifetime of the engagement pipeline. Every outreach draft must be checked against this list before send.
