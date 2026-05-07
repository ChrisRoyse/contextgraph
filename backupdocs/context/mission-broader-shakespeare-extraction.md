---
namespace: context
agent_id: coordinator
timestamp: 2026-04-18T04:30:00Z
status: active
tags: mission, shakespeare, real-text-only, sft-expansion, no-synthetic
---

# Mission: Broader Shakespeare SFT Extraction (NO SYNTHETIC DATA)

## Why (supersedes prior "Resurrection DPO via Gemma" plan)
User flagged correct concern: LLM-generated REJECTED data creates model-collapse risk. We have 2,698 real Shakespeare memories in Context Graph + 249K multi-dimensional signals — that's plenty of human-written source material to expand SFT without any synthetic generation.

Current SFT corpus: 7,925 examples
Target: **15,000+ SFT examples** from real Shakespeare passages

## Non-negotiables
- **NO synthetic / LLM-generated CHOSEN text.** Every SFT example's assistant response must be a verified Shakespeare passage from his actual works.
- **NO Gemma generation** as positive signal. Gemma rejected-response pipeline is KILLED.
- **Use Context Graph's 13-embedder signals** to guide extraction — pick passages that are STYLISTICALLY CONCENTRATED (high E7/E9/E10/E12/E13, style-preserving embedders).
- **Fail-fast** on CG daemon unreachable or schema mismatch.
- **FSV**: verify extracted examples are real Shakespeare by spot-checking against known source texts.

## Target deliverables
- `~/.contextgraph/training/shakespeare_sft_broader.jsonl` — 15,000+ SFT examples, schema matches existing `shakespeare_sft_final_clean.jsonl`
- All passages must be:
  - Non-empty, ≥ 40 chars
  - From the 2,698 CG memories (or related extractions of Shakespeare's complete works)
  - No duplicates (same passage text) with existing SFT
  - Paired with diverse prompts (multiple angles per passage)
- Source provenance tracked per example: `{sft_index, source_play, memory_id, stylistic_tags}`

## Constraint from upstream (Phase B stylistic analyzer output)
We have `~/.contextgraph/training/shakespeare_stylistic_tags.jsonl` (7,925 Phase B tags). These can GUIDE which passages are under-represented (low prose vs. high couplet coverage, etc.).

## Phase Plan (sequential subagents, SMAP coordination)

### Agent A: Shakespeare-extraction-surveyor (researcher)
Survey the 2,698 Shakespeare memories in Context Graph:
1. Query daemon for ALL Shakespeare memories via `list_training_records` / `search_by_embedder` E1 on "shakespeare"
2. Cross-reference with existing SFT corpus to find un-extracted passages
3. Compute stylistic distribution via Phase B tags
4. Write discoveries + report to `./memory/`
5. Propose extraction strategy for Agent B

**Writes to**: `memory/discoveries/agent-A-survey-*.md`, `memory/reports/agent-A-*.md`

### Agent B: Broader-extractor-coder (coder)
Reads Agent A's report + existing `scripts/shakespeare_extract*.py` patterns. Builds new extractor script:
- `scripts/shakespeare_extract_broader_sft.py`
- Pulls from CG via MCP tools
- Generates 15,000+ SFT examples with diverse prompts
- Output schema matches existing clean file
- FAIL-FAST on any CG error
- Idempotent + resumable

**Writes to**: `memory/discoveries/agent-B-coder-*.md`, `memory/reports/agent-B-*.md`

### Agent C: Code-simplifier (review)
Reviews Agent B's script for redundancy, correctness, edge cases.

**Writes to**: `memory/reports/agent-C-simplifier-*.md`

### Coordinator (me) — FSV after each agent
- After Agent A: verify the report + discoveries against actual CG data
- After Agent B: code-review, syntax-check, execute small sample, verify output schema
- After Agent C: apply refinements, run full extraction, FSV output count + content sample

## Services state
- Daemon on port 3100 UP, PID 52941 (52+ min uptime) — MUST stay up
- Inference server: OFF (not needed for real-text extraction)
- GPU: 16.8 GB used (daemon) / 15 GB free
- Gemma generator: KILLED

## Related
- `./memory/context/mission-resurrection-plan.md` (SUPERSEDED — synthetic path deprecated)
- `./memory/context/mission-brief-shakespeare-guard-hardening.md` (layered on top, still valid)
- `./memory/decisions/coordinator--guard-architecture.md` (still valid)
- Existing data: `~/.contextgraph/training/shakespeare_sft_final_clean.jsonl` (7,925)
- Existing data: `~/.contextgraph/training/shakespeare_stylistic_tags.jsonl` (7,925)
- Existing scripts: `scripts/shakespeare_extract*.py` (reference patterns)
