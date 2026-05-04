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

- AI platform engineers shipping governed agents across teams.
- CISOs and security architects who need containment, audit trails, and proof.
- Regulated enterprise teams that need evidence for model and agent controls.
- Agent vendors that need a trust layer for customer deployments.
- SRE, DevOps, SecOps, and data platform teams using agents for operational work.

## Core Differentiator

Most observability tools show that an action happened. BlackBoxFix proves
whether the intended outcome actually persisted where it matters.

The defensible claim is the **expected delta / actual readback** pair. Every
mutating action ships with both, and the verifier compares them physically:

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

In Phase 1 (shipped) the ledger persists this pair structurally; Phase 2/3 add
the connectors that produce `actual_readback`.

## Product Principles

- **Source-of-truth first.** Return values are never enough. Verifiers read
  the durable system on a separate path.
- **Fail closed.** Unverified state is a blocking state in governed mode.
  Phase 1 already enforces this for the ledger itself: any redaction,
  schema, hash-chain, or readback failure aborts the write and exits non-zero.
- **Typed failures.** Every failure has `error_code`, `field_path`, and
  `remediation` (Phase 1 codes enumerated in `20_phase1_state_and_invariants.md`).
- **Real systems only.** Verification reads actual systems, databases, files,
  APIs, ledgers, or other durable state. No mock connectors, ever.
- **Evidence over trust.** Users see the state before, the trigger, the state
  after, and the verifier conclusion.
- **Training by operation.** Every verified action becomes usable supervision
  data via the Phase 5 episode exporter.

## Non-Goals

- Do not build another generic trace viewer as the main product.
- Do not store full secrets, credentials, or unredacted sensitive payloads.
  (Phase 1 enforces this with the `redact_v1` policy + post-scan; secret-like
  literals outside redacted fields → `BBX_REDACTION_VIOLATION`.)
- Do not silently skip verifiers when credentials, APIs, or sources of truth
  are unavailable. `blocked` is not success.
- Do not accept a model's explanation as proof of success.
- Do not make safety dependent on prompt text alone.
- Do not introduce migration shims when schemas change. Wipe the ledger.

## Jobs To Be Done

- "Before I allow agents to change production, prove they only did what was
  authorized and prove the final state is correct."
- "When an agent breaks something, give me the exact action chain, source of
  truth evidence, and remediation path."
- "Turn real agent work into evals so every regression can be caught before
  deployment."
- "Give compliance teams exportable evidence for AI agent controls."

## Long-Term Possibility

BlackBoxFix can become the operational memory and proof layer for tool-using
AI. The richer the corpus of verified state/action/outcome episodes becomes,
the better it can train agents to reason about real consequences instead of
only textual plausibility.

## Verification Of This Document

Source of truth: this file plus `20_phase1_state_and_invariants.md`. Edge case
to confirm: the `BBX_REDACTION_VIOLATION` path actually rejects an agent that
posts a `Bearer ...` token in a non-sensitive field. Reproduce with the
synthetic input in `20_phase1_state_and_invariants.md` §"Edge Case 1".
