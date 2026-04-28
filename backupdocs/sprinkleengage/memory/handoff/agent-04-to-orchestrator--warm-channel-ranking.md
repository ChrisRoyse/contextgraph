---
namespace: handoff
agent_id: agent-04-warm-channels
timestamp: 2026-04-19T13:50:00Z
status: active
tags: handoff, orchestrator, warm-channels, ranking
---

# Handoff: agent-04 → orchestrator (warm-channel ranking input)

## What the orchestrator should use from this wave

### For the TARGET_UNIVERSE_RANKING "Accessibility" column
Use the following scoring (verified affiliations as of April 2026):

| Target org | Accessibility score (1–5) | Justification | Best named contact |
|---|---|---|---|
| Anthropic | 4 | Ethan Perez + MATS Summer 2026 is a published outsider channel; Chris Olah replies publicly | Ethan Perez (A1.4) |
| Google DeepMind | 3 | Delétang is reachable via compression-thesis paper; Hassabis is not | Grégoire Delétang (A3.5) |
| OpenAI | 2–3 | Boris Power (applied-research inbound) or Khosla (investor) only | Boris Power (A2.1) |
| Meta MSL | 3 | Alexandr Wang high-profile but Wang's bandwidth = low; Marco Baroni (FAIR) + Angela Fan are reachable | Marco Baroni (A4.4) |
| Mistral | 3 | Anjney Midha sits on Mistral's board; Lacroix/Lample are co-founders but bandwidth is scarce | Anjney Midha (F6) via board |
| Databricks | 4 | Zaharia won ACM Prize 2026; Khattab (MIT, ex-Databricks research) is a direct bridge | Matei Zaharia (E6) + Omar Khattab (E1) |
| Apple, Amazon, xAI | — | NOT COVERED in this pass. Recommend follow-up pass. |

### For the "Recommended In" column
1. Anthropic → MATS Summer 2026 application with Ethan Perez
2. Google DeepMind → cold paper email to Grégoire Delétang citing compression-thesis overlap
3. OpenAI → Boris Power applied-research email; backup: Khosla investor angle (low odds)
4. Meta MSL → Marco Baroni FAIR Paris academic-channel email
5. Mistral → warm intro via Anjney Midha (a16z/AMP PBC, Mistral board)
6. Databricks → Matei Zaharia direct; use Khattab ACM Prize 2026 as opener
7. Anthropic (alternate) → X public reply to Chris Olah on interp/verifiability thread

### Highest-leverage single first move (orchestrator's "one named person, one channel, one week-1 action")
**Send one cold email to Elvis Dohmatob (Concordia) offering EXP-2 pairwise-MI co-author collaboration.**

- Probability of reply: VERY HIGH (active X poster, new-professor network-building posture, highly-cited 2024/25 collapse papers).
- Leverage: a single endorsement from Dohmatob validates the paper's scope-argument against Shumailov-form collapse (paper §2.5/§3.1). That endorsement is the *technical key* that unlocks both (a) a clean path to Shumailov/Kempe/Feng validation, and (b) a much better reception from the VCs in §F who all index on "does a collapse-paper author think this is real".

See `memory/decisions/agent-04-warm-channels--top-5-ins.md` for the ranked 5 ins with offer language.

---

## What's NOT in this wave (orchestrator should request follow-up if needed)

1. **GitHub/arXiv cross-pollination (Category H)** — requires a Semantic Scholar / OpenAlex API pass + HuggingFace follower-graph extraction. Not done here.
2. **Apple, Amazon, xAI, Cohere, Qwen/Alibaba, DeepSeek, Inflection-remnants warm channels** — not covered; brief focused on frontier-lab + collapse-cluster + VC.
3. **Sovereign wealth (Tier C)** — not covered here; likely a separate agent's territory.
4. **Specific 2026 paper acceptances at NeurIPS/ICML/COLM** — recommend another agent pull these program lists once published (COLM March deadline passed April).
5. **Verification pass on single-sourced claims** flagged in `discoveries/agent-04-warm-channels--collapse-cluster-triangulation.md` (Kempe→EIT Oxford, Shumailov current affiliation).

---

## Dependencies the orchestrator should track

- Paper status: must be ready to send (PDF, HuggingFace LoRA, CI disclosure, Case-scope tags) before any outreach in `top-5-ins.md`.
- CI disclosure footer mandatory on every outbound artefact.
- Dohmatob outreach should reference BOTH "A Tale of Tails" and "Strong Model Collapse" to signal the paper author has actually read them.

---

## Files written this wave
```
research/00_target_universe/04_warm_channels_map.md
memory/decisions/agent-04-warm-channels--top-5-ins.md
memory/discoveries/agent-04-warm-channels--collapse-cluster-triangulation.md
memory/discoveries/agent-04-warm-channels--khattab-mit-retrieval-bridge.md
memory/handoff/agent-04-to-orchestrator--warm-channel-ranking.md
memory/journal/agent-04-warm-channels--start.md
memory/reports/agent-04-warm-channels--final-report.md
```

## Competing Interests Disclosure
> Chris Royse is founder/operator of Teleox.ai (primary) and Leapable.ai (companion MCP marketplace). This package emerges from commercial engineering practice. Readers should weight content as a methodology contribution with acknowledged commercial affiliation.
