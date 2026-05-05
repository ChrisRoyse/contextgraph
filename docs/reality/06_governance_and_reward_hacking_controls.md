# Governance And Reality-Boundary Controls

Autonomous optimization is only useful if the agent cannot cheat the benchmark, weaken the verifier, or promote itself on false evidence.

In ContextGraph's implemented reality loop, the model does not own the reward path. The model can propose a candidate action, but the Rust loop, DynamicJEPA state machinery, BlackBoxFix ledger, source-of-truth readbacks, and official evaluator own the measurement path. That changes the risk model: the primary failure mode is not ordinary prompt-level reward hacking, but a reality-boundary defect where the system measured the wrong thing, measured too narrow a slice of reality, or allowed an unsafe editable scope.

## Implemented Reality Boundary

For the Rust SWE reality loop, the current boundary is:

- the model receives task context and returns a candidate patch;
- the model cannot directly edit the BlackBoxFix ledger, reward compiler, official evaluator, verifier definitions, Docker harness, or task manifest;
- the system validates that the patch applies to the base repository;
- the system blocks benchmark test, fixture, pytest, tox, and harness mutations before claiming progress;
- official success is read from the SWE-bench `resolved_ids` report, not from model text;
- reward is compiled from persisted ledger and scorecard rows;
- every attempt writes prompt, response, parse report, patch, validation, verifier, score, reward, and event artifacts to durable files or SQLite.

So the model cannot "hack the scalar" by changing the scalar. If the loop produces a false success, that is a bug in measurement scope, verifier implementation, source-of-truth selection, or policy configuration, and must be treated as a ContextGraph defect.

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

## Reality-Boundary Risks

These are the remaining risks after the reward path is outside model control.

| Risk | Control |
|---|---|
| The source of truth is narrower than the intended outcome | Name the source of truth explicitly and avoid claims beyond it. For SWE-bench, "resolved" means official test reality, not proof of all possible behavior. |
| The verifier points at the wrong repository, commit, container, or report | Persist commit SHA, Docker image, report path, report SHA-256, and official `resolved_ids` readback. |
| The deterministic verifier has a bug | Version, hash, and review verifier code separately from model-editable artifacts. |
| The model finds a production-code special case that passes visible tests without fixing intent | Use held-out tasks, adversarial tests, semantic regression suites, and follow-up source-of-truth probes. |
| The loop optimizes development tasks but fails promotion tasks | Separate dev smoke, curriculum, and held-out promotion suites. |
| The outer loop weakens prompts or context to hide failure | Persist prompt compiler hashes, prompt SHA-256, model response SHA-256, and before/after scoreboards. |

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
