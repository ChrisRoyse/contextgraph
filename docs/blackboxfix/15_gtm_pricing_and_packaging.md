# Go-To-Market, Pricing, And Packaging

## Category

Position BlackBoxFix as **Verified AgentOps**:

- not just LLM observability;
- not just evals;
- not just policy gateway;
- not just audit logs.

The category claim is **operational proof for AI agent actions** — every
action shipped with an expected delta and a physical readback.

## Initial Wedge

DevOps/SecOps agent governance:

- coding agents;
- dependency remediation agents;
- CI repair agents;
- cloud change agents;
- vulnerability triage agents.

This wedge is concrete, budgeted, and verifiable.

## Buyer Personas

| Persona | Cares about |
|---|---|
| AI platform lead | reusable controls for many agent teams; SDK, APIs, eval exports, model-provider neutrality |
| CISO / security architect | authorization, audit, incident response, SIEM, data controls |
| Engineering leader | shipping agents without breaking production |
| Agent vendor | enterprise trust, embedded SDK, customer proof |

## Offer

30-day pilot:

- one workflow;
- one agent runtime;
- first connector set (Git + GitHub + CI + scanner);
- evidence dashboard;
- training/eval export;
- security review packet.

Pilot success metric:

- ≥ 20 real agent actions recorded;
- 100 % of mutating actions verified or blocked;
- ≥ 1 failed/blocked episode converted into an eval;
- buyer can replay a run and inspect source-of-truth evidence.

## Pricing Hypotheses

| Tier | Mechanism |
|---|---|
| Developer | free or low-cost local recorder (Phase 1 today); JSONL exporter; local Git/file verifiers |
| Team | per active agent **or** per verified run volume; shared dashboard; GitHub/CI/ticket integrations |
| Enterprise | annual platform fee + usage tier for verified tool calls; private deployment premium; retention premium; custom connector package; compliance/audit export package |
| Vendor / OEM | SDK license; per end-customer deployment; white-label enterprise controls |

## Metrics That Sell

- percentage of agent actions verified;
- number of blocked unsafe actions;
- mean time to root cause;
- evals generated from real failures;
- reduction in manual audit work;
- number of workflows allowed to move from supervised to bounded-autonomous.

## Sales Proof (live demo script)

1. Agent performs useful operational work.
2. BlackBoxFix records the action.
3. Source-of-truth readback proves success.
4. A controlled failure produces a typed incident.
5. The failure becomes an eval/training row.

Use the wedge demo in `18_first_wedge_demo.md` end-to-end. Always show the
actual SQLite query / `git log` / GitHub API response — never a screenshot.

## Expansion Paths

- add more connectors;
- move from observe-only to governed mode;
- expand from DevOps to data operations;
- expand from one agent team to enterprise platform;
- sell training/eval corpus export to model improvement teams.

## What Not To Sell Yet

Until each phase lands, do not promise:

- live MCP gateway (Phase 6);
- local dashboard bundle (Phase 7) and hosted dashboard later;
- enterprise SSO/RBAC/private deploy (Phase 8);
- non-Git/non-GitHub connectors (Phase 3+).

Selling unbuilt capability turns Phase 0/1 evidence into a credibility
liability. Stick to what is physically demoable.

## FSV Of GTM Claims

A GTM claim is FSV-passing only if it can be supported by:

1. A reproducible CLI command sequence the buyer can run today.
2. The exact `sqlite3 …` query that proves the state.
3. A code path under `crates/context-graph-cli/` that produced the artifact.

If any of those three is missing, the claim is unverified and must not appear
in collateral.
