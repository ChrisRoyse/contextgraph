# 10 Agent Workflow Learning

## Purpose

Make real agent runs trainable without bypassing the existing Learning-as-UTL
storage path.

The operational loop is:

```text
real event
-> append-only workflow ledger
-> RocksDB CF_LEARNING_EVENTS
-> preference pairs
-> candidate policy
-> holdout benchmark
-> promote only if candidate is better
```

## Source Of Truth

| Artifact | Default path | Role |
| --- | --- | --- |
| workflow ledger | `~/.contextgraph/agent_workflow_learning/events.jsonl` | Lossless event trace: query, contexts, tool calls, final response, physical evidence, outcome. |
| projected learning events | `~/.contextgraph/agent_workflow_learning/learning_events.jsonl` | `LearningEvent` rows ready for CLI/RocksDB import. |
| RocksDB learning events | user-provided `--storage`, CF `learning_events` | Durable training source consumed by existing Phase 6 tools. |
| preference pairs | `~/.contextgraph/agent_workflow_learning/workflow_pairs.jsonl` | Chosen/rejected workflow comparisons mined only from real contrasted outcomes. |
| candidate policy | `~/.contextgraph/agent_workflow_learning/policy_candidate.json` | Learned workflow/retrieval/tool feature weights. |
| active policy | `~/.contextgraph/agent_workflow_learning/active_policy.json` | Promoted policy. Written only after benchmark improvement. |

## Event Capture

Create an event JSON with:

- `session_id`, `task_id`, `user_query`, `final_response`
- `retrieved_contexts`: source, rank, text, used flag, relevance
- `tool_calls`: name, success, verified, purpose, summaries
- `physical_evidence`: at least one check such as `file_exists`, `file_contains`, `json_file_value`, `command`, or `rocksdb_count`
- `outcome_signal`: label, utility delta, accepted/corrected/reused flags

Then record it:

```bash
python3 scripts/v4_2/agent_workflow_learning.py record \
  --event /path/to/event.json \
  --storage /path/to/rocksdb
```

The command fails before writing if a training-eligible event has failing
physical evidence. When `--storage` is set, it imports into
`CF_LEARNING_EVENTS` and immediately reads the event back through
`context-graph-cli`.

## Pair, Train, Benchmark, Promote

```bash
python3 scripts/v4_2/agent_workflow_learning.py build-pairs --overwrite

python3 scripts/v4_2/agent_workflow_learning.py train-policy \
  --min-events 10 \
  --overwrite

python3 scripts/v4_2/agent_workflow_learning.py benchmark-policy \
  --split holdout \
  --overwrite

python3 scripts/v4_2/agent_workflow_learning.py promote \
  --min-accuracy 0.55 \
  --min-improvement 0.01 \
  --overwrite
```

Promotion fails closed unless the candidate beats the baseline by the requested
margin on held-out preference pairs.

## MCP Learning Signal Tools

The deterministic Learning-as-UTL embedders are exposed directly through MCP:

- `list_learning_signal_embedders`: lists `delta_e`, `surprise`, `coherence`, `consolidation`, and `transfer`.
- `compute_learning_signals`: computes selected signals from an inline before/after/outcome transition without storage mutation.
- `embed_learning_event_signals`: reads a persisted `LearningEvent` from `CF_LEARNING_EVENTS`, re-embeds selected signals, and reports whether they match persisted signals.

Use `embed_learning_event_signals` when physical proof matters because it reads
the stored RocksDB row and returns before/after `learning_events` counts.

## Full-State Verification

Run:

```bash
python3 scripts/v4_2/agent_workflow_learning.py run-fsv
```

The FSV harness creates a real RocksDB store, records four real workflow events,
prints before/after state for each happy path and edge case, exports learning
events, mines pairs, trains a candidate, benchmarks against a baseline, and
promotes only if better.

Latest report path:

```text
~/.contextgraph/reports/agent_workflow_learning_fsv_latest.json
```
