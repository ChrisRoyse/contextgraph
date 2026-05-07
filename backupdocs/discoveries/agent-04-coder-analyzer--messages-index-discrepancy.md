---
namespace: discoveries
agent_id: agent-04-coder-analyzer
timestamp: 2026-04-17T16:20:00Z
status: active
severity: medium
tags: sft-format, shakespeare, messages-index, phase-b
---

# Shakespeare SFT passage lives at messages[2], NOT messages[1]

## What Happened
Task brief said "Extract `messages[1].content` as the Shakespeare passage (the assistant's response)." But in every sample inspected from `~/.contextgraph/training/shakespeare_sft_final_clean.jsonl`, the message layout is:
- `messages[0]` role=system  ("You are a knowledgeable assistant who writes EXCLUSIVELY...")
- `messages[1]` role=user    (the prompt to answer)
- `messages[2]` role=assistant (the Shakespeare passage to analyze)

Verified across records 1, 100, 1000, 5000, 7900 — all have n_msgs=3 with the same role ordering.

## Decision
The analyzer extracts `messages[-1].content` (last message) and asserts its role is `assistant`. If any record has a different shape, it's logged to error log and emitted with `analyzer_error: "invalid_message_shape"`. This matches the brief's unambiguous "the assistant's response" wording over the likely-off-by-one "messages[1]".

## Why It Matters
If we had followed `messages[1]` literally, EVERY output tag record would describe the USER PROMPT, not the Shakespeare passage. The DPO generator (Phase C) and SFT augmenter (Phase D) both consume these tags assuming they reflect Shakespeare style — using the wrong content would destroy the entire downstream pipeline.

## Related
- ./memory/decisions/coordinator--stylistic-tag-schema.md (schema)
- ./memory/context/mission-resurrection-plan.md (B → C/D dependency)
- Tags: sft-format, messages-index, phase-b
