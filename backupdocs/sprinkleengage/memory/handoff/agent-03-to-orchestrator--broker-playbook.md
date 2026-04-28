---
namespace: handoff
agent_id: agent-03-deal-brokers
timestamp: 2026-04-19T02:25:00Z
status: active
tags: handoff, orchestrator, brokers, precedents, teleox
---

# Handoff: Agent-03 Deal-Brokers → Orchestrator

## What the orchestrator needs from this wave

### 1. Precedent-distilled structural choices (inputs to TARGET_UNIVERSE_RANKING)
For each Tier-A/B hyperscaler, the **structural preference** is now visible from precedents:

| Hyperscaler | Observed Structure | Banker Pattern | Back-Channel |
|---|---|---|---|
| Meta | **49% non-voting + CEO-to-Meta** (Scale AI, 2025) | Centerview (sole) | CEO-to-CEO (Wang–Zuckerberg) |
| Google/Alphabet | **License + reverse acqui-hire** (Character 2024) OR traditional M&A (Mandiant 2022, Wiz 2025) | Self-advised <$10B; Goldman >$10B | DeepMind leadership → Shazeer precedent |
| Microsoft | **License + talent absorption** (Inflection 2024) OR traditional M&A (Nuance 2022) | Goldman for traditional; **Reid Hoffman direct-to-Nadella** for acqui-hire | Hoffman (Microsoft director) |
| Amazon | **Reverse acqui-hire into AGI team** (Adept 2024) | Not disclosed | Rohit Prasad (AGI SVP) |
| Apple | No public $1B+ AI acqui-hire precedent | — | — |
| Nvidia | Strategic investment rather than acquisition | — | — |

### 2. Broker ranked recommendations
See `decisions/agent-03-deal-brokers--top-3-brokers.md`:
- **#1 Qatalyst (Quattrone) — cold-viable**
- **#2 LionTree (Bourkoff) — cold-viable**
- **#3 Centerview — warm-intro required**

### 3. Key unknowns to flag for orchestrator
- **No warm intro path to Centerview** visible in current memory state. If Meta is the #1 target, orchestrator must task downstream agent to map Chris's LP/advisor network for Centerview-adjacent connections.
- **No Reid-Hoffman equivalent** for Teleox. If MSFT is the #1 target, the Hoffman-path (board-director-introducer) is not currently available; default to Goldman-TMT formal-M&A at LOI stage.
- **Quattrone's minimum deal floor is unpublished** but implied ~$2B+. Below that floor, the default sell-side path is Moelis or Evercore.

### 4. Patterns worth elevating to `patterns/` namespace
- **Named-researcher proof anchor dominates acqui-hires** → elevated in `discoveries/agent-03-deal-brokers--research-paper-first-pattern.md`
- **Advisor invisibility on acqui-hires** (banks not disclosed for Character/Inflection/Adept) → same doc
- **Regulator-scrutiny-first structuring** (every L&A drew inquiry) → same doc

## What agent-03 recommends the orchestrator do next

### Cross-reference with agent-01 (frontier labs) and agent-02 (hyperscalers) outputs
If agent-01 ranks Anthropic #1, the broker path is **unknown** — Anthropic has no public $1B+ acqui-hire (they are the acquirer side in their structures). The bank path would require identifying Anthropic's corp-dev head.

If agent-02 ranks Meta #1, the broker path is **Centerview** (confirmed from Scale precedent). **Warm-intro is the binding constraint.**

If agent-02 ranks Amazon #1, the broker path is **Rohit Prasad direct**. No bank needed.

### Sequence inputs for TARGET_UNIVERSE_RANKING.md
- Capacity column: all Tier-A can write $3B+; Tier-B requires hyperscaler-backed syndication.
- Accessibility column: **Qatalyst, LionTree = cold-viable**; **Centerview, Allen, Goldman = warm-required**.
- Recommended "In" column: for each top target, match to broker recommendation above.
- Confidence column: HIGH for Meta-path (Centerview confirmed); MEDIUM for Amazon (Prasad confirmed but access unverified); LOW for Apple (no precedent).

## Files written (MEMORY KEYS WRITTEN — required by SMAP)
- `research/00_target_universe/03_deal_brokers_and_precedents.md` (main deliverable)
- `memory/discoveries/agent-03-deal-brokers--research-paper-first-pattern.md`
- `memory/decisions/agent-03-deal-brokers--top-3-brokers.md`
- `memory/handoff/agent-03-to-orchestrator--broker-playbook.md` (this file)
- `memory/reports/agent-03-deal-brokers--final-report.md` (final action)

## Competing Interests Disclosure
Chris Royse is founder/operator of Teleox.ai (primary) and Leapable.ai (companion MCP marketplace). This package emerges from commercial engineering practice. Readers should weight content as a methodology contribution with acknowledged commercial affiliation.
