# Big Unlock Plan

This directory turns the verified Shakespeare corpus run into an implementation plan for the full 21-slot system:

- E1-E14: content-side fingerprints, already proven on the complete Shakespeare corpus.
- E15-E21: learner-state channels, stored beside content fingerprints in learner-specific RocksDB CFs.
- Learning events: the bridge between content retrieval/generation and learner outcomes.
- Training exports: the repeatable path from persisted observations to DPO/reranker/diagnostic datasets.

The goal is not to add a new architecture. The goal is to connect the pieces that now exist:

1. The Shakespeare corpus is a real, multi-embedder content substrate.
2. Learner state tools and CFs exist.
3. State-conditioned retrieval policy exists.
4. Learning-event export and learner-conditioned training datasets exist.
5. MCP can now expose a capability matrix and high-throughput real batch ingestion.

The missing operational step is to attach real learner observations to real retrieval/generation sessions, then prove with full state verification that E15-E21 change retrieval, outcomes, and training data.

## Documents

| File | Purpose |
| --- | --- |
| [00_current_state.md](00_current_state.md) | What is proven now, with source-of-truth locations and gaps. |
| [01_target_capabilities.md](01_target_capabilities.md) | What the 21-slot Shakespeare+learner system should do once wired. |
| [02_data_model_and_sources.md](02_data_model_and_sources.md) | Learner-state data sources, E15-E21 mapping, provenance, consent, and calibration. |
| [03_end_to_end_architecture.md](03_end_to_end_architecture.md) | Trigger-to-source-of-truth dataflow across content, learner state, learning events, and training. |
| [04_mcp_tool_workflows.md](04_mcp_tool_workflows.md) | Concrete MCP workflows for registration, observation, retrieval, outcome logging, export, and training. |
| [05_state_conditioned_retrieval.md](05_state_conditioned_retrieval.md) | How E15-E21 should affect Shakespeare retrieval and generation decisions. |
| [06_closed_loop_training.md](06_closed_loop_training.md) | How learner outcomes become DPO/reranker/diagnostic training data. |
| [07_full_state_verification.md](07_full_state_verification.md) | Required FSV harnesses, physical CF checks, edge cases, and success evidence. |
| [08_implementation_roadmap.md](08_implementation_roadmap.md) | Ordered implementation plan with file/module ownership and acceptance criteria. |
| [09_risk_and_policy_gates.md](09_risk_and_policy_gates.md) | Fail-closed, privacy, consent, and scientific validity gates. |

## One-sentence target

Build a Shakespeare tutor/generator that retrieves and generates from a proven E1-E14 Shakespeare substrate while conditioning every retrieval, recommendation, learning event, and training export on persisted E15-E21 learner-state observations, with RocksDB and MCP full-state verification at every step.

