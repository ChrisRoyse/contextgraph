---
namespace: journal
agent_id: agent-A-surveyor
timestamp: 2026-04-17T22:30:00Z
status: active
tags: start, survey, shakespeare, extraction, sft-expansion
---

# Agent A (Surveyor) — Start Journal

## Context Absorbed
- Read all 8 mandatory SMAP files (mission, resurrection plan, guard brief, guard architecture, constellation anchors, search schema, SMAP protocol, SITREP)
- Mission: inventory 2,698 Shakespeare memories in Context Graph, cross-reference with the existing 7,925 SFT examples, propose extraction strategy for Agent B to reach 15,000+ SFT examples WITHOUT synthetic content.
- Superseded mission: the DPO resurrection plan (Phase C was going to synth rejected variants via Gemma — KILLED. New approach: real-text-only SFT expansion).
- Existing data confirmed on disk:
  - `~/.contextgraph/training/shakespeare_sft_final_clean.jsonl` → 7,925 lines (35 MB)
  - `~/.contextgraph/training/shakespeare_stylistic_tags.jsonl` → 7,925 lines (38.5 MB, from Phase B analyzer)
- Daemon should be UP on 127.0.0.1:3100 (SITREP notes user restarted host; will verify via MCP call)

## Plan
1. Verify daemon is up & responsive (daemon_status / list_constellations probe)
2. Use `count_training_records` if available, else `list_training_records` pagination
3. Sample SFT JSONL to understand schema & source provenance keys
4. Sample stylistic_tags JSONL for tag distribution
5. Inventory CG memories per play/source via constellation membership + search_by_embedder
6. Cross-reference: which memories are represented? Which are under-extracted?
7. Compute gap estimate (potential new examples per source)
8. Propose concrete extraction strategy with prompt-template diversity
9. Write discoveries + final report

## Constraints I Will Honor
- Read-only — DO NOT modify any data, DO NOT write SFT
- No synthetic content proposals
- Fail-fast — if CG returns unexpected, investigate & report
- Frontmatter on every memory file
