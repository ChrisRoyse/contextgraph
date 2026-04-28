---
namespace: patterns
agent_id: agent-47-graph-gardener
timestamp: 2026-04-17T15:05:00Z
status: active
supersedes: none
tags: pruning, archive, cross-refs, non-destructive, graph-hygiene
---

# Pruning Recommendations

## Hard rule restated

**No hard deletes.** Per the agent-47 mandate and the project `constraints.md` rule "Do NOT delete blocker files." All recommendations below are **archive-only** or **cross-reference additions**. The downstream coordinator retains final editorial control.

## TL;DR

The memory graph is **clean**. There are no orphan files, no duplicate locked decisions, and no contradictory constructs. The only graph-hygiene actions worth taking are:

1. **Correct 3 role labels** in handoff files 43, 44, 45 (LOW effort, LOW risk).
2. **Mirror unresolved stop-ship items** into `blockers/` for visibility (LOW effort, MEDIUM value).
3. **Add one cross-reference** in the coordinator context so STOP-8 / STOP-3 status is tracked into the final assembly.

No files require archival. No files require merging.

## Category A — Files to archive (NONE)

No file in `decisions/`, `discoveries/`, `patterns/`, `handoff/`, or `reports/` is orphaned or superseded. The memory graph is at its working size (206 files) and every file is either a load-bearing locked decision, an analysis artifact referenced by at least one writer-agent or reviewer-agent, or a handoff that anchors the sequential chain.

Archive recommendation: **NONE.**

## Category B — Cross-references to add (3 surgical edits, optional)

These are OPTIONAL additions the coordinator or agent-48 may apply. They do not alter content, only improve navigability.

### B1 — Correct role labels in handoffs 43, 44, 45

Three handoff files refer to "agent-47 (final-assembler)" but the canonical role (per `context/agent-order.md`) is agent-47 = graph-gardener. The actual final assembly is done by the coordinator post-agent-48.

- `handoff/agent-43-to-44--reproducibility-ready.md` line 171 — change "Agent-47 (final-assembler)" to "Coordinator (post-agent-48)".
- `handoff/agent-44-to-45--references-ready.md` lines 107, 233 — same correction.
- `handoff/agent-45-to-46--citations-validated.md` lines 120, 168 — same correction.
- `handoff/agent-46-to-47--length-audit-ready.md` lines 94, 134 — change "Agent-48 (final-assembler)" to "Coordinator (post-agent-48)".

**Severity: LOW.** Does not affect section content. Adjust only if the coordinator wants a clean handoff graph for future readers of the memory tree.

### B2 — Mirror unresolved stop-ship items into `blockers/`

Per `constraints.md` §7 ("Document every blocker in `./memory/blockers/`"), the following should have been blocker files. Agent-47 recommends that agent-48 file these, NOT as retroactive blocker-writes by agent-47 (because agent-47 cannot resolve them) but as visible unresolved-work tracking:

- `blockers/stop-8-camera-ready-language--UNRESOLVED.md` — 6 section files still contain "camera-ready"; agent-42 STOP-8 directive not applied. Severity HIGH.
- `blockers/stop-3-temperature-sweep-claim--UNRESOLVED.md` — §6.3.3 paste-ready edit unapplied; agent-43 flagged the literal edit as introducing an unsubstantiated two-run claim. Needs author decision. Severity HIGH.
- `blockers/case3-table2-source-mismatch--UNRESOLVED.md` — §6.3.4 Table 2 headline numbers do not reconcile with `alldata.md` §5.5 ablation-internal values. Severity HIGH.
- `blockers/case3-n14-ambiguity--UNRESOLVED.md` — §6.3.2 N=14 ambiguity (clone N=14 / real N=1). Severity MEDIUM.
- `blockers/ic6-asymmetric-release-label--UNRESOLVED.md` — IC-6 asymmetric-release posture not labelled in §6 body; agent-43 specified 3 one-sentence inserts. Severity MEDIUM.
- `blockers/section15-reproducibility-appendix--REQUIRED.md` — agent-43 authored a §15 paste-ready draft; it has not been added to `../sections/`. If this is shipped with arXiv v1, sample size and reference discipline need a second validation pass. Severity MEDIUM for v1, HIGH for camera-ready.

**Severity of the action: MEDIUM value, LOW effort.** These are not NEW findings — they were discovered by agents 42 and 43 and are traceable from their `discoveries/` files. Mirroring into `blockers/` makes them easier for the coordinator and agent-48 to see at a glance.

### B3 — Add a graph-gardener note to `context/`

**Optional.** If the coordinator wants agent-48 to receive an explicit cross-reference linking the agent-47 inventory and pruning files to the agent-48 briefing, a one-file insert at `context/current-phase.md` (or a new `context/agent-47-graph-gardener--note.md`) would anchor agent-48's reading list to include this agent's outputs. Equivalent to what agent-47 writes in its handoff below — this is redundant but would make navigation faster.

**Severity: LOW.** Not required. Agent-47's handoff (`handoff/agent-47-to-48--memory-graph-ready.md`) already names its four output files as agent-48's reading list.

## Category C — Blockers to close (NONE possible by agent-47)

The `blockers/` directory is empty. There are no filed blockers to close. The unresolved stop-ship items surfaced above are NOT filed blockers — they are discoveries/pattern entries from agents 42 and 43 that the coordinator has not yet acted on. Closing them requires section-file edits by the author/coordinator and is outside agent-47's scope.

## Category D — Constructs or decisions to merge (NONE)

All 18 locked decisions are disjoint by construct or artifact class. The 5 agent-04 construct files (DDA, meaning-compression, TCT, identity-locked generation, support constructs) cover distinct Tier-A or Tier-B glossary entries. No merge is warranted.

All 21 non-locked `decisions/` entries (including the 18 locked + 3 informational) are single-subject files. No merge is warranted.

## Category E — Files needing integrity verification

These files were referenced by downstream agents but not individually re-verified by this agent:

1. `decisions/agent-08-citation-extractor--master-reference-pool.md` — agent-44 treats this as canonical. Agent-45 concurs 63 entries in the published reference list. Not audited by agent-47.
2. `patterns/agent-22-model-architect--variable-dictionary.md` — writers use this as vocabulary source. Not audited.
3. `patterns/agent-27-instrument-developer--instrument-to-case-map.md` — methodology-writer and results-writer rely on this. Not audited.

**Severity of the action: LOW.** These files are structurally sound (well-formed frontmatter, referenced by downstream handoffs). Deep content verification is not agent-47's scope.

## What agent-47 does NOT recommend

1. **Do NOT delete any memory file.** Not even "obviously redundant" ones. Later reviewers may want the trail.
2. **Do NOT rewrite handoffs to redirect role labels** — just correct the three specific labels noted in B1, not restructure the handoff chain.
3. **Do NOT retroactively file blocker files ABOUT pipeline run-time issues** that agent-47 hypothesizes but cannot verify. File blockers only for the 6 concrete stop-ship items listed in B2, each of which has a verifiable grep test.
4. **Do NOT compress the section files** — agent-46 explicitly deferred that decision to the coordinator. Agent-47 concurs with that deferral.

## Confidence

- **HIGH** that no files need to be archived.
- **HIGH** that the role-label corrections in B1 are correct (direct grep + canonical order check).
- **HIGH** that STOP-8 camera-ready is unresolved (grep verified).
- **MEDIUM** that STOP-3 / Table-2 / N=14 issues are unresolved (agent-43's discoveries file says so; not independently re-verified against `alldata.md`).
- **HIGH** that no merges are warranted.

## Changelog

- 2026-04-17 — Recommendations written by agent-47-graph-gardener.
