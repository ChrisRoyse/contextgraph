# Governance And Reward-Hacking Controls

Autonomous optimization is only useful if the agent cannot cheat the benchmark, weaken the verifier, or promote itself on false evidence.

## Immutable Boundaries

The optimization loop must not be allowed to directly modify:

- verifier definitions used for promotion;
- official benchmark harness;
- held-out task set;
- production reward spec;
- policy gates;
- source-of-truth connectors;
- ledger hash-chain logic;
- promotion thresholds;
- secrets and credentials;
- audit export code.

Changes to those areas require a separate governed review path.

## Editable Scopes

The agent can modify only approved scopes:

- candidate patch;
- prompt candidate;
- tool-call plan;
- temporary experiment branch;
- non-production reward proposal;
- model adapter candidate;
- local run metadata.

Every scope must be declared in the policy decision.

## Reward-Hacking Risks

| Risk | Control |
|---|---|
| Agent edits tests instead of code | Official harness and base commit readback. |
| Agent weakens verifier | Verifier directory is outside editable scope. |
| Agent repeats lucky patch | Patch hash repeat detection and no-repeat prompt constraints. |
| Agent overfits visible tasks | Held-out task split and promotion gate. |
| Agent optimizes scalar while increasing risk | Component-level reward, risk penalties, policy gates. |
| Agent hides failure logs | Independent file readback and ledger hash verification. |
| Agent fabricates success | Success requires source-of-truth readback. |
| Agent burns compute indefinitely | Fixed budget, timeout, cost tracking, stop rules. |

## Signed Reward Specs

Reward specs should be content-addressed:

```json
{
  "reward_id": "contextgraph_verified_outcome_demo_v1",
  "sha256": "sha256:...",
  "source_of_truth": "file:/.../reward_spec.json#sha256",
  "signature": "optional-sigstore-or-local-signature"
}
```

The ledger should store the reward spec hash used for every reward compilation.

## Promotion Gate

A run can be promoted only if:

- source-of-truth coverage is complete;
- mandatory verifiers pass;
- policy status is allowed;
- artifact hashes match;
- no critical incident exists;
- held-out tasks pass when applicable;
- reward exceeds current champion;
- reward spec hash matches an approved spec;
- verifier spec hash matches an approved verifier.

## Human Steering

Borrow Ralph's steering idea, but make it auditable:

```text
docs/reality/STEERING.md
```

Each loop should read this file at iteration boundaries and persist:

- steering file SHA-256;
- applied steering rule;
- whether the rule changed model prompt, task priority, or budget.

## Failure Modes Must Be Productive

Every failure should become one of:

- typed error;
- incident packet;
- training negative;
- eval regression;
- prompt compiler constraint;
- policy rule candidate;
- connector reliability finding.

Never let a failure vanish as a console log.

## No False Certainty

The system should never claim:

- "solved" without official or source-of-truth proof;
- "safe" without policy and verifier evidence;
- "trained" without a durable training episode;
- "improved" without a before/after reward comparison;
- "generalized" without held-out task evidence.
