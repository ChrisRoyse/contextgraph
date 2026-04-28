---
namespace: patterns
agent_id: agent-00-orchestrator
timestamp: 2026-04-19T00:30:00Z
status: active
tags: smap, conventions, file-discipline, naming
---

# SMAP Conventions for This Project

## Directory Map
```
/home/cabdru/contextgraph/docs/sprinkleengage/
├── memory/                        # SMAP shared mind
│   ├── context/                   # mission-brief, constraints (orchestrator-written)
│   ├── decisions/                 # locked architectural / strategic choices
│   ├── discoveries/               # constraints, gotchas, edge cases
│   ├── handoff/                   # agent-N → agent-N+1 deliverables
│   ├── journal/                   # append-only execution log
│   ├── blockers/                  # unresolved issues
│   ├── patterns/                  # conventions (this file)
│   └── reports/                   # each agent's final report (mandatory)
└── research/                      # the actual deliverables
    ├── 00_target_universe/        # Wave 1 scan outputs
    ├── 01_company_research/       # Phase 1 (future, if deep-dive invoked)
    ├── 02_strategic_positioning/
    ├── 03_conversation_engineering/
    ├── 04_sales_enablement/
    ├── 05_executive_synthesis/
    └── 06_meta/
```

## File Naming
```
{namespace}/agent-{N}-{role}--{descriptor}.md
```
Examples:
- `decisions/agent-01-frontier-labs--primary-target.md`
- `discoveries/agent-03-brokers--allen-co-pattern.md`
- `handoff/agent-01-to-orchestrator--ranked-frontier-list.md`
- `journal/agent-04-warm-channels--success-linkedin-map.md`
- `reports/agent-02-hyperscalers--final-report.md`

## Mandatory Frontmatter
```yaml
---
namespace: {context|decisions|discoveries|handoff|journal|blockers|patterns|reports}
agent_id: agent-{N}-{role}
timestamp: {ISO 8601, e.g. 2026-04-19T08:30:00Z}
status: {active|resolved|superseded}
severity: {high|medium|low}     # required for blockers
tags: {comma-separated keywords}
---
```

## Lifecycle (READ → ORIENT → WORK → WRITE → REPORT)
1. **READ** `./memory/` first: context → decisions → patterns → discoveries → blockers → handoff → reports → journal.
2. **ORIENT**: identify what binds you, what prior agents found, what next agents need.
3. **WORK**: write `.md` files to `./memory/` IMMEDIATELY as you discover / decide / block / pattern / fail / succeed. Also write your research deliverable under `./research/`.
4. **WRITE** handoff for the next agent.
5. **REPORT**: last action is writing `./memory/reports/agent-{N}-{role}--final-report.md` with full audit trail.

## Hard Rules
1. Always READ memory before working. Never skip.
2. Never contradict locked decisions.
3. Never hide failures — write to `journal/` or `blockers/`.
4. Never finish without final report.
5. Always include frontmatter.
6. Always tag paper-derived claims with paper section (§2.5, §3.1, etc.).
7. MEASURED/PROJECTED tags on every Case-derived number.
8. CI disclosure in every research/*.md footer.

## Reporting Back to Orchestrator (CRITICAL)
Every agent's final message to the orchestrator MUST include:
```
MEMORY KEYS WRITTEN:
- memory/discoveries/agent-{N}-{role}--{desc}.md
- memory/decisions/agent-{N}-{role}--{desc}.md
- memory/handoff/agent-{N}-to-{next}--{desc}.md
- memory/reports/agent-{N}-{role}--final-report.md
- research/{subdir}/{filename}.md
```
Without this list, the orchestrator cannot coordinate. Agents that omit it will be re-spawned.

## File Length
- Max 1500 lines per file; split to `_part1.md`, `_part2.md` with cross-refs.
- Research deliverables: aim 500–1500 lines of substance.
- Memory files: aim 50–300 lines; only go longer for final reports.

## Scope Preservation Reminders (paper §8)
- Case 3 = encoder-matched, N=10, one speaker, English.
- "Up to 91 signals/input" = constructive upper bound, EXP-2 pending.
- DDA outside Shumailov = scope, not refutation.
- TCT ≠ superior to RLHF — different property.
- Case 2 = **MEASURED and operational** (Chris confirmed 2026-04-19). Paper draft §6.2 tag is paper-internal only; outreach treats Case 2 as measured and available to demonstrate. Subject artefacts remain withheld (dual-use).
