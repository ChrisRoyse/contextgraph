---
namespace: decisions
agent_id: agent-document-architect
created: 2026-05-09T00:00:00Z
updated: 2026-05-09T00:00:00Z
status: active
severity: high
tags: directive, operator, mejepa, reality-loop, claude-code, swe-bench, single-loop, supersedes-two-loop
---

# Operator directive 2026-05-09: Single reality loop, Claude Code is the SWE-bench actor, reality loop is ME-JEPA's training data factory

## What changed (locked decision)

The previous "outer optimizer + inner repair actor" two-loop architecture is
**superseded**. The system has ONE reality loop, and that loop exists for
exactly one purpose: **generate training data for ME-JEPA**.

| Field | Old (superseded) | New (locked 2026-05-09) |
|---|---|---|
| Reality loops | TWO — outer (Claude Code optimizer) + inner (Qwen3.6 repair actor) | **ONE** — outer reality loop around Claude Code |
| SWE-bench actor | Qwen3.6-27B (local model) | **Claude Code** itself |
| Purpose of the loop | Optimize harness so Qwen converges | **Capture every reward signal from durable reality so ME-JEPA can be trained on it** |
| ME-JEPA's role | One of several deliverables | **The product. The only thing that matters.** |
| LLM ↔ ME-JEPA | LLM and ME-JEPA tightly coupled (verifier wraps LLM) | **ME-JEPA runs SEPARATE from the LLM** — it is a stand-alone reality predictor |
| End goal | When ME-JEPA-vs-oracle correlation ≥ 0.95, stop reviewing AI patches | **When ME-JEPA can predict, for ANY dynamic project / ANY supported language, whether code works in reality, software engineers stop reviewing code** |

## What this changes in the docs / architecture

1. **Delete inner-loop framing.** No more "outer optimizer drips Qwen attempts." The loop is "Claude Code attempts SWE-bench tasks; reality loop captures the data; ME-JEPA learns from the data."
2. **Reality loop = data factory.** Every Claude Code action (issue selection, file open, edit, test run, oracle verdict, drift event, hook payload) is reward signal for ME-JEPA. Capture all of it.
3. **ME-JEPA's three operator-stated questions** (verbatim):
   - *Does what the AI said it did exist?* (claim ↔ source-of-truth SHA chain)
   - *Does it work as intended?* (predicted oracle ↔ actual oracle)
   - *How is it actually impacting reality?* (predicted side-effect set ↔ observed shifts)
4. **Cross-language / cross-project transfer is mandatory.** SWE-bench Lite 300 is the bootstrap; bigger benchmarks come after. ME-JEPA must work on any dynamic project in any of the supported languages (Python, Rust, C, C++, Java, C#, JS/TS, Ruby per `09 §3.1` AST harness state).
5. **Self-healing and self-learning are first-order requirements**, not optional features.
6. **UTML v1.0 (`L_step = ΔP × ΔK × ΔΩ × ΔΞ`) is the canonical learning formula** for ME-JEPA — see `decisions/2026-05-09--utml-v1-jepa-edition-mejepa-learning-formula.md`. The earlier 3-term `L = ΔS × ΔC × ΔE` from UTL v2.0 (decision file `2026-05-09--utl-v2-mejepa-learning-formula.md`) is HISTORICAL and was superseded when `docs2/UTL_v2_embodied.md` was rewritten on 2026-05-09 from "UTL v2.0 — Embodied Edition" to "UTML v1.0 — JEPA Edition" (four ML-native factors). The central architectural claim: ME-JEPA's `ΔΞ.target_collapse = 0 by construction` because targets come from frozen externally-defined instruments — uniquely structurally collapse-immune among JEPA-class systems.

## Operator quotes (verbatim, 2026-05-09)

> "the whole purpose of this system is to build the me jepa that can predict if
> code works or not so software engineers no longer have to review code to
> ensure what the ai said it did it actually did."

> "we can actually remove any reality loop stuff all together. the reality loop
> system is solely to get training data for the me jepa system. that ist he
> whole purpose of the reality loop."

> "we just need to optimize the outer loop, the one reality loop around claude
> code. we will be using claude code to pass the swe bench."

> "we will be using all the issues and all the runs and all the data from
> everything claude code does to feed the me jepa and store all the traiing data
> so we can build out a system fully capable of in any dynamic project with any
> of the coding languages supported the me jepa will be able to predict if the
> system will work in reality or not."

> "this system can derive reward signals from durable reality. that is what we
> are doing, trying to capture all the reward signals as possible so we can have
> as much data as possible for the me jepa to be able to predict reality with."

> "we go one test at a time. gain all training data and the system learns and
> improves with each loop."

> "it is very important this system is self healing and self learning."

## Why this matters for future sessions

A new session opening this repo MUST treat the single-loop / Claude-Code-as-actor
/ ME-JEPA-as-product framing as the **organizing principle** for every technical
decision. If `CLAUDE.md`, `loop.md`, `09_END_GOAL_REVIEW_REPLACEMENT.md`, or any
sitrep doc still uses the older two-loop framing, **trust this decision over the
stale wording** and refactor the doc to match. Use `status: superseded` markers
on the old framing rather than silent rewrites.

## How to apply

- Reality loop docs (`docs/ccreality/`, `.claude/loop.md`, `docs/ruvectorfindings/`)
  refer to **Claude Code** as "the SWE-bench actor", not the "outer optimizer."
  The "optimizer" role is now: optimize the data-capture machinery itself
  (instruments, hooks, shift-log, witness chain) so ME-JEPA gets richer signal
  per attempt.
- Inner-actor identity guardrails (`RUST_REALITY_INNER_MODEL_*`) become a
  **deprecated code path** that should be replaced with a Claude-Code-attempt
  capture path. Until the replacement code lands, the existing engine still
  drips Qwen attempts; treat each such attempt as legacy training data with the
  inner-actor flag set.
- The 8 mutation categories in `09 §3.1` remain the bootstrap corpus. Their
  generation is unchanged — only the inner-loop framing of how they're
  consumed is replaced.
- Cross-language coverage (Python / Rust / C / C++ / Java / C# / JS / TS /
  Ruby) is now a Tier-1 requirement, not Tier-3.

## Related

- `decisions/2026-05-09--utml-v1-jepa-edition-mejepa-learning-formula.md` (canonical companion file — the explicit four-factor `L_step = ΔP × ΔK × ΔΩ × ΔΞ` formula for ME-JEPA training).
- `decisions/2026-05-09--utl-v2-mejepa-learning-formula.md` (HISTORICAL — the earlier 3-term form, kept per SMAP rule 8 "superseded decisions stay").
- `decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`
  (clean-room rule unchanged; still in force).
- `context/end-goal-verifiable-ai-code.md` (predates this directive but aligns
  with it).
- `context/mission-brief.md` (must be re-issued to reflect the single-loop
  framing).

## Tags

`directive` `operator` `mejepa` `reality-loop` `single-loop` `claude-code`
`swe-bench-actor` `data-factory` `cross-language` `self-healing`
`self-learning` `supersedes-two-loop`
