---
namespace: context
agent_id: agent-00-orchestrator
timestamp: 2026-04-19T00:30:00Z
status: active
tags: mission, brief, target-universe-scan, 3b-threshold, teleox, tct
---

# Mission Brief — Target Universe Scan for $3B+ Engagement

## Pivot (supersedes prior Anthropic-only brief)
Chris's directive: research **all model makers + deal brokers** capable of writing a **$3B+ check**
for Teleox.ai's Meaning Compression / TCT / DDA substrate, and identify the **single best "in"** —
one named person, one intro path, one first engagement move.

Anthropic remains a candidate, but is no longer the default target. The default is **ranked
discovery across the full universe**.

## Principal
- **Name**: Chris Royse
- **Company**: Teleox.ai (primary) + Leapable.ai (companion MCP marketplace)
- **Email**: chrisroyseai@gmail.com
- **Public artefacts**:
  - Paper (April 2026): *Teleological Constellation Training...*
  - LoRA: huggingface.co/cabdru/shakespeare-lora-gemma4
  - Context Graph (~370K LOC Rust) + ClipCannon (~67K LOC Python)
  - Case 3 measured: 0.961 mean WavLM SECS (within-WavLM-family, N=10, 1 speaker, English)

## Transaction Threshold: $3B+
This is acquisition / mega-licensing / category-defining-strategic-investment territory, not
ordinary corpus licensing. Comparables:
- Scale AI → OpenAI-led $14.3B valuation round (2024)
- Character.AI → Google ~$2.7B acqui-hire (2024)
- Inflection → Microsoft ~$650M data+team (2024)
- Adept → Amazon acqui-hire (2024)
- Nuance → Microsoft $19.7B (2022)
- Mandiant → Google $5.4B (2022)

Implication: only hyperscalers, mega-cap tech, top-5 frontier labs with hyperscaler backing, and
sovereign wealth funds can write this check.

## Target Universe (to be researched)

### Tier A — Hyperscalers / Mega-Cap (cash + strategic fit)
- Alphabet / Google DeepMind
- Microsoft (+ OpenAI via partnership)
- Meta AI
- Amazon (+ Anthropic via $8B investment)
- Apple
- Nvidia

### Tier B — Frontier Labs (strategic urgency + data-wall literacy)
- OpenAI
- Anthropic
- Mistral AI
- xAI
- Cohere
- Qwen / Alibaba
- DeepSeek
- Inflection remnants → MSFT
- Perplexity / Character / lower-cap labs (likely sub-$3B outcomes)

### Tier C — Sovereign / Strategic Capital
- UAE G42 / Mubadala
- Saudi PIF / Humain
- Qatar Investment Authority
- Singapore Temasek / GIC
- SoftBank Vision Fund
- Norway NBIM (sovereign index holder; not acquirer)

### Tier D — Deal Brokers (accelerants, not buyers)
- Investment banks: Allen & Co, Qatalyst, Centerview, LionTree, Goldman TMT, Morgan Stanley TMT,
  Evercore, Moelis, JPM TMT
- Boutique AI M&A advisors
- Board members / venture partners with portfolio cross-pollination
- Research-publication-as-entry: NeurIPS, ICLR, ICML, arXiv mentions leading to corp dev contact

## Core Question for the Research Pipeline
For each Tier A/B/C candidate, answer:
1. **Capacity**: Can they write $3B+? (Yes/No/Syndicated)
2. **Strategic fit**: How aligned with TCT/DDA/MC thesis?
3. **Recent AI M&A / mega-licensing signals** (last 24mo)
4. **Named decision-makers** for a deal of this shape (CEO, Chief Scientist, Head of Alignment,
   Chief Business Officer, Head of Corp Dev)
5. **Warm-channel "in"** — specific named people who could introduce, specific conferences they
   attend, specific podcasts/publications they respond to

For Tier D, answer:
1. Which bankers / advisors ran the Character.AI, Inflection, Adept, Scale AI deals?
2. Who advises on frontier-lab transactions?
3. Accessibility: who responds to a cold paper + LoRA outreach?

## Final Deliverable Shape
`research/00_target_universe/TARGET_UNIVERSE_RANKING.md`

Columns:
| Rank | Target | Tier | Capacity | Strategic Fit | Accessibility | Recommended "In" | Confidence |
|---|---|---|---|---|---|---|---|

Plus: single recommended first move (one named person, one channel, one week-1 action).

## Thesis Recap — Primitive-First Framing (Chris-confirmed 2026-04-19)

**The primitive:** a fixed corpus of `n` inputs projected through `N` frozen approximately-
independent embedders yields **up to `n·(N+C(N,2))` structured supervisory signals — up to 91 at
N=13**, a counting identity (paper §0.2). **Every new embedder added to the pool multiplies
correlations against every existing embedder. Public embedder inventory (~13 today) is the
current constraint; the primitive is extensible without bound — an embedder can represent the
meaning of anything a function can compute.**

This is the first step of the AI pipeline — the data. A third path around the Villalobos data
wall, structurally outside the Shumailov-form recursion regime (scope, not refutation, §3.3).
Per-output geometric verifiability via `G_τ` (§5.4) is a byproduct of the primitive.

**The three case studies in the paper (Shakespeare LoRA, Santa talking-head, voice clone) are
exploratory proofs of what becomes possible with this primitive — not products being pitched.**
Every outreach in this package leads with the primitive; case studies come second as empirical
anchors. See `research/05_executive_synthesis/THE_PRIMITIVE.md` for the full statement.

## Non-Negotiable Tone (paper §0.5)
- MEASURED ≠ claimed everywhere it is (Case 3 within-WavLM scope; DDA upper-bound hedge)
- "Structurally outside Shumailov regime" = scope, not refutation
- No commercial TAM claims from Teleox
- CI disclosure in every deliverable footer

## Pipeline (revised)
| Wave | Agents | Deliverable | Dependency |
|---|---|---|---|
| 1 (parallel) | 4 research agents | Tier-A/B scan + Tier-C scan + Tier-D brokers + precedent deals | none |
| 2 (orchestrator) | me | TARGET_UNIVERSE_RANKING.md synthesis | wave 1 |
| 3 (user decision) | Chris | pick #1 target | wave 2 |
| 4 (future) | sprinkle 22-doc pipeline | full engagement package on #1 target | wave 3 |

## SMAP compliance
Every agent MUST read `./memory/` first, write outputs to `./memory/` and `./research/`, and report
memory keys written in their final message. Universal template lives in
`./memory/patterns/agent-00-orchestrator--smap-conventions.md`.
