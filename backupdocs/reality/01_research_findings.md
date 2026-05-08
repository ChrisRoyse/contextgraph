# Research Findings

## Ralph Loop

Ralph Loop is a long-running AI development loop. The public docs describe a system that starts from a PRD/task list, runs an agentic CLI inside Docker sandboxes, executes tests/typecheck/screenshot checks, records history, and repeats until tasks are complete or an iteration limit is reached.

The useful ideas for ContextGraph:

- A durable task source of truth is better than a one-shot prompt.
- Each task should have explicit success criteria.
- Work should run in an isolated sandbox.
- Iterations need history logs, timing, progress state, and live observability.
- A steering file lets a human redirect the loop without destroying history.
- Promise/status tags such as complete, blocked, and decide are useful control-plane signals.

The weakness relative to ContextGraph:

- Ralph-style loops usually treat tests/logs/task status as enough. ContextGraph should go further: each mutating action needs expected delta, actual readback, verifier result, evidence packet, scorecard, reward, and replayable provenance.

## Karpathy AutoResearch

Karpathy AutoResearch is the closest research pattern to what ContextGraph should become. The repository gives an AI agent a small real training setup, a file it can modify, a fixed time budget, a measurable metric, and a keep-or-discard rule.

Important AutoResearch primitives:

- The system under optimization is real, not mocked.
- Only one file is editable in the baseline design, keeping scope small and diffs reviewable.
- The metric is mechanical: validation bits per byte, lower is better.
- Every experiment has a fixed time budget, making attempts comparable.
- The agent records results in a structured table.
- Better experiments advance the branch; equal/worse experiments are discarded.
- The prompt program is itself treated as an artifact the human can improve over time.

ContextGraph should generalize this:

- Instead of only `val_bpb`, use verified outcome reward from source-of-truth readbacks.
- Instead of one training file, use governed tool scopes.
- Instead of only git keep/revert, use ledger-backed experiment selection and artifact provenance.
- Instead of trusting a run log, verify filesystem, database, Docker, git, test, and ledger state independently.

## SWE-Agent And Agent-Computer Interfaces

SWE-agent argues that agents need interfaces designed for agents, not raw human terminals. Its Agent-Computer Interface work highlights command design, succinct observations, repository navigation, editing tools, and test execution as major drivers of performance.

ContextGraph should treat its tools the same way:

- expose narrow, typed tools;
- return compact source-of-truth observations;
- avoid dumping entire logs when a verifier can extract the actionable failure;
- make every command produce a structured event;
- reject malformed tool calls before mutation;
- run verifiers on a separate read path.

## SWE-Bench

SWE-bench provides real GitHub issues and official Docker-based evaluation. For ContextGraph, the crucial point is that leaderboard claims must be tied to the official report, especially whether the instance id appears in `resolved_ids`.

ContextGraph should use local validation as an early signal only. The authoritative stop condition for SWE-bench is official Docker readback.

## Karpathy Software 2.0 / 3.0 Framing

Karpathy's Software 2.0 framing says much of the program is represented by data, architecture, weights, and optimization. Modern Software 3.0 framing extends that idea to natural-language prompts, tools, and agents.

ContextGraph adds the missing compiler target:

```text
prompt/code/action -> real system outcome -> verified reward -> training/eval data
```

The source code of the agent is no longer just weights or prompts. It is the full reality loop:

- task specification;
- tool contract;
- policy;
- verifier;
- reward spec;
- source-of-truth readback;
- prompt compiler;
- training episode exporter;
- promotion gate.

## Synthesis

The strongest design combines:

- Ralph's long-running task loop and observability;
- Karpathy AutoResearch's fixed-budget experiment discipline;
- SWE-agent's agent-specific tool interface;
- SWE-bench's official real-world benchmark readback;
- ContextGraph's BlackBoxFix/DynamicJEPA evidence, reward, provenance, and source-of-truth verification.

That produces a system where models do not just retry. They retry with verified reality as their feedback signal.
