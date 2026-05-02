# Market And Buyer Research

## Market Thesis

Enterprises want agents for coding, operations, support, compliance, security,
analytics, and back-office workflows. The blocker is not only model quality. The
larger blocker is permission to let agents act in systems where mistakes create
security, financial, compliance, or availability risk.

BlackBoxFix monetizes that blocker by making agent actions inspectable,
verifiable, replayable, and exportable as training/eval data.

## Urgent Buyer Pain

AI platform teams need a shared control plane for agent actions across model
providers and frameworks.

CISOs need evidence for:

- what tools the agent invoked;
- what data the agent accessed;
- whether the action was authorized;
- whether the claimed result exists in the system of record;
- what happened when the verifier failed.

Regulated enterprises need durable audit evidence that maps to risk management
and governance frameworks.

Agent vendors need enterprise trust features to close deals: audit logs, policy
controls, tenant isolation, redaction, and verifiable outcomes.

## Why This Can Be Profitable

The product sits near high-budget concerns:

- production access control;
- security governance;
- regulated audit evidence;
- model risk management;
- developer productivity;
- incident response;
- training data generation.

The buyer can justify spend because the alternative is delaying agent rollout or
allowing uncontrolled agents into expensive workflows.

## Initial ICP

Best first customers:

- 200 to 5000 engineer companies adopting coding or DevOps agents;
- regulated SaaS, fintech, healthcare, defense-adjacent, infrastructure, and
  cyber companies;
- AI platform teams responsible for multiple internal agent projects;
- security teams evaluating autonomous code, remediation, or cloud agents.

Avoid initial ICPs where the source of truth is ambiguous, primarily human, or
hard to inspect. DevOps/SecOps workflows produce concrete evidence quickly.

## Competitive Landscape

Adjacent categories:

- LLM observability and prompt tracing;
- AI gateway and policy enforcement;
- runtime security and data loss prevention;
- model eval platforms;
- SIEM/SOAR;
- audit logging and governance risk compliance tooling;
- agent frameworks with built-in traces.

BlackBoxFix should integrate with these categories instead of copying them. The
defensible product claim is verifiable outcome evidence plus training/eval
corpus generation.

## Positioning

Short form:

> Verified AgentOps: every agent action recorded, replayed, verified, scored,
> and converted into training/eval data.

Security buyer form:

> A black box recorder and source-of-truth verifier for autonomous AI actions.

Platform buyer form:

> A governance and evaluation layer for tool-using agents across frameworks.

## First Wedge Offer

"30-day verified agent pilot":

- one high-value workflow;
- one agent runtime;
- three to five mutating tools;
- physical source-of-truth readback;
- dashboard and evidence export;
- eval corpus generated from real runs.

Example workflow:

1. Agent remediates a dependency vulnerability.
2. BlackBoxFix records the issue, plan, file edits, tests, PR, CI run, and
   scanner result.
3. Verifiers read Git diff, lockfile, test output, CI status, scanner state, and
   ticket state.
4. The platform exports episodes for future evals and regression tests.

## Packaging

- Developer: local recorder, CLI, file/Git verifier, JSONL exporter.
- Team: shared ledger, GitHub/CI/ticketing connectors, dashboards.
- Enterprise: SSO, RBAC, tenant isolation, retention, SIEM export, private
  deployment, custom connectors.
- Vendor/OEM: SDK and embedded verification layer for agent products.

