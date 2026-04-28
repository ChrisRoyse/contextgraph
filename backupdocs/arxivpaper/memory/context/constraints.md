---
namespace: context
agent_id: agent-00-coordinator
timestamp: 2026-04-16T15:10:00Z
status: active
tags: constraints, non-negotiable, arxiv
---

# Global Constraints (NON-NEGOTIABLE — agents MUST respect)

## Output format
- **Every agent writes prose sections or analysis notes as Markdown to `./memory/` namespaces** per SMAP.
- Agents that write manuscript sections also write the **final section text** to `../sections/{section-name}.md`.
- No code-blocks of Python or Rust inside the manuscript body. Pseudocode and equations only. Code goes in appendix.

## Agent behavior rules
- **Ignore the `npx claude-flow` hook commands in every agent's frontmatter.** Those commands are not installed in this environment. Use the file-based `./memory/` protocol instead.
- **Do NOT execute the pre/post hooks as shell commands.** Treat them as informational only.
- Every agent's LAST action is a final report at `./memory/reports/agent-{N}-{role}--final-report.md`.
- Every agent writes a handoff at `./memory/handoff/agent-{N}-{role}--{descriptor}.md` describing what the next agent should read.

## Citation and evidence
- Every empirical claim cites a source.
- The four existing evidence briefs are the **primary source pool**:
  - `docs/researchpaper/_evidence_01_data_wall.md` — scaling, collapse, compression
  - `docs/researchpaper/_evidence_02_voice_cloning.md` — TTS SOTA and SECS
  - `docs/researchpaper/_evidence_03_video_embedders.md` — video diffusion + embedders
  - `docs/researchpaper/_evidence_04_enterprise_market.md` — licensing deals + industry signals
- All URLs already verified. Do NOT re-verify unless you suspect drift.
- APA 7th edition formatting throughout.

## arXiv-specific constraints
- Paper body ≤ 30 pages when typeset. Target ~8K–12K words of prose.
- References: 30–60 entries (all verified URLs).
- No anonymisation — Chris Royse is the author.
- Primary category `cs.LG`. Cross-listings `cs.CL`, `cs.CV`.
- Must be submittable by arXiv's standard .tex/.pdf pipeline, but we ship the source as Markdown — translation to LaTeX happens later.

## No marketing in the body
- Teleox.ai and Leapable.ai go in Acknowledgements and a brief commercial-disclosure footnote.
- Do not write "the best" / "revolutionary" / "breakthrough" / "game-changing" — arXiv reviewers penalize marketing language.
- Do not make claims about AGI. We reframe the data wall; we do not solve intelligence.

## Empirical claim discipline
- **Measured** = verified numbers in prior report (Case 3 is the only one fully in this category at submission time).
- **Architecturally complete, measurement pending** = system runs but specific metric not yet produced (Case 1 Shakespeare retrain; Case 2 Santa LoRA). Flag explicitly with "(projected; camera-ready will re-verify)".
- **Constructive evidence** = system exists as running code. Cite file paths and line counts.
- Never present projected numbers as measured.

## Failure documentation
- Per SMAP rule 7: failures are as valuable as successes. Document every blocker in `./memory/blockers/`.
- If an agent cannot complete its task, it still writes a final report explaining why, with severity and suggested remediation.

## Handoff discipline
- Every agent names the SPECIFIC memory files the NEXT agent should read first.
- No general "see ./memory/" directives — give file paths.

## What NOT to do
- Do NOT contradict locked decisions in `./memory/decisions/`.
- Do NOT delete blocker files — update `status: resolved` instead.
- Do NOT silently fail. Write a blocker file.
- Do NOT rewrite prior agents' outputs. Build on them; if an earlier output is wrong, file a decision overriding it with rationale.
