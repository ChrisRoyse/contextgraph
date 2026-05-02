# Product Vision

BlackBoxFix is a Verified AgentOps platform for organizations that want AI
agents to act in real systems without losing control, auditability, or training
signal. It is the operational layer between an agent runtime and the systems
the agent can affect.

## Product Statement

Every AI agent action is:

- recorded with enough context to explain intent and authorization;
- verified against the source of truth after execution;
- scored for correctness, risk, and policy compliance;
- replayable for audit, incident response, and debugging;
- converted into training and eval data.

## Primary Users

- AI platform engineers who need to ship governed agents across teams.
- CISOs and security architects who need containment, audit trails, and proof.
- Regulated enterprise teams that need evidence for model and agent controls.
- Agent vendors that need a trust layer for customer deployments.
- SRE, DevOps, SecOps, and data platform teams using agents for operational work.

## Core Differentiator

Most observability tools show that an action happened. BlackBoxFix proves
whether the intended outcome actually persisted where it matters.

The key distinction is the expected delta/readback pair:

```json
{
  "expected_persisted_delta": {
    "git.commit": "created",
    "ci.workflow": "passed",
    "ticket.status": "resolved"
  },
  "actual_readback": {
    "git.commit": "9b21...",
    "ci.workflow": "passed",
    "ticket.status": "resolved"
  },
  "verifier_status": "passed"
}
```

## Product Principles

- Source-of-truth first: return values are never enough.
- Fail closed: unverified state is a blocking state for governed workflows.
- Typed failures: every failure has an error code, location, and remediation.
- Real systems only: verification reads actual systems, databases, files, APIs,
  ledgers, or other durable state.
- Evidence over trust: users should see the state before, the trigger, the
  state after, and the verifier conclusion.
- Training by operation: every verified action becomes usable supervision data.

## Non-Goals

- Do not build another generic trace viewer as the main product.
- Do not store full secrets, credentials, or unredacted sensitive payloads.
- Do not silently skip verifiers when credentials, APIs, or sources of truth are
  unavailable.
- Do not accept a model's explanation as proof of success.
- Do not make safety dependent on prompt text alone.

## Jobs To Be Done

- "Before I allow agents to change production, prove they only did what was
  authorized and prove the final state is correct."
- "When an agent breaks something, give me the exact action chain, source of
  truth evidence, and remediation path."
- "Turn real agent work into evals so every regression can be caught before
  deployment."
- "Give compliance teams exportable evidence for AI agent controls."

## Long-Term Possibility

BlackBoxFix can become the operational memory and proof layer for tool-using AI.
The richer the corpus of verified state/action/outcome episodes becomes, the
better it can train agents to reason about real consequences instead of only
textual plausibility.

