---
namespace: journal
agent_id: agent-07-coder-patches
timestamp: 2026-04-17T19:15:00Z
status: active
tags: journal, start, patches, phase-b, phase-c, phase-d
---

# Agent 7 starting: applying Sherlock S3 critical patches

## Plan
Apply the following patches in order:
- P1 Phase B (analyzer) — managed_run wrapper, spacy doc release, error log rotation
- P2 Phase C (DPO builder) — generator for SFT records, managed_run, tag index trim, budget flag
- P3 Phase D (augmenter) — D-3 resume check both partial+final, D-2 subprocess isolation, D-1 batched generate, managed_run

Imports verified: `from shakespeare_resource_guard import managed_run` OK on Python 3.13.9.

Goal: surgical edits, preserve happy path, minimize blast radius.
