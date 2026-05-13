---
namespace: reports
created: 2026-05-13T00:34:15Z
updated: 2026-05-13T00:34:15Z
status: active
tags: report, session, mejepa, phase-e, github-issues, planning, fsv
---

# Session Report: 2026-05-13 - Phase E GitHub Issue Plan

## Task

Create atomic GitHub issues for every implementation item needed for Phase E to be complete. The issues are the source of truth for the next execution sequence.

## Outcome

Complete. GitHub had no open issues before this pass. Created 18 open `[Phase E]` issues, numbered #20 through #37, covering HEAL-001 through HEAL-008, UTML-005 through UTML-008, TASK-SEC-005, witness-chain integrity scheduling, and the final end-to-end FSV/docs closure gate.

## Source-of-Truth Verification

- Before state: `gh issue list --repo ChrisRoyse/contextgraph --state open --limit 200 --json number,title,labels,url` returned `[]`.
- After state: GitHub readback returned 18 open `[Phase E]` issues with numbers `20..37`.
- Common label verification showed no missing `source:agent`, `agent:codex`, `status:needs-triage`, or `priority:p1` labels.
- Body verification showed no missing `## Acceptance / Full State Verification` section.
- Coverage verification found all expected tokens present across the issue set: `HEAL-001` through `HEAL-008`, `UTML-005` through `UTML-008`, `TASK-SEC-005`, and `REQ-HEAL-FB-15`.

## Issues Created

- #20 `[Phase E] 01 - Implement durable self-optimization policy ledger and scheduler orchestration`
- #21 `[Phase E] 02 - Enrich drift/reward event schema with cell, head, session, and surprise metadata`
- #22 `[Phase E] 03 - Implement per-cell stratified drift detection and intervention queueing`
- #23 `[Phase E] 04 - Implement Bayesian small-window drift posterior`
- #24 `[Phase E] 05 - Implement surprise-weighted drift and active-learning fast path`
- #25 `[Phase E] 06 - Replace synthetic promotion holdouts with real holdout/eval telemetry source`
- #26 `[Phase E] 07 - Implement A/B/C scoring formula and per-cell promotion gate`
- #27 `[Phase E] 08 - Implement catastrophic promotion approval queue and MCP tool`
- #28 `[Phase E] 09 - Enforce two-person catastrophic promotion security policy`
- #29 `[Phase E] 10 - Implement global and per-head dissipation detection`
- #30 `[Phase E] 11 - Persist per-head Fisher snapshots for EWC++`
- #31 `[Phase E] 12 - Implement per-head dormancy detection and targeted reinitialization`
- #32 `[Phase E] 13 - Implement per-head lambda ramp policy`
- #33 `[Phase E] 14 - Implement production constellation refresh policy`
- #34 `[Phase E] 15 - Validate emergency eviction production policy and alerting`
- #35 `[Phase E] 16 - Schedule witness-chain integrity checks and write-freeze repair policy`
- #36 `[Phase E] 17 - Implement drift attribution and operator alert stream`
- #37 `[Phase E] 18 - Complete Phase E end-to-end FSV suite and documentation closure`

## Research Notes

External production ML guidance supports the issue split:

- Production model monitoring should capture request/response data, compare to baselines, detect drift, and alert on deviations.
- Continuous evaluation should join ground-truth labels with predicted labels when labels are available, and identify which request types perform well or poorly.
- Safe production promotion should use automated quality gates, staged/canary or A/B evaluation, rollback paths, and formal human approval for high-impact promotion.

## Next Order

Start with #20, then #21. The remaining issues depend on those durable policy/event sources being implemented before detectors, promotion, approval, and final FSV can be made reliable.
