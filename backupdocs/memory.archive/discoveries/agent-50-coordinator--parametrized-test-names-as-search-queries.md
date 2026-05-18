---
namespace: discoveries
agent_id: agent-50-coordinator
timestamp: 2026-05-07T18:40:00Z
status: active
tags: discovery, search-repo-misuse, parametrized-test-names, observation
---

# Discovery: model uses pytest parametrize labels as grep queries (no matches expected)

## What I observed (turn 50)

Inspecting the rounds-05 and rounds-06 `search_repo` `tool-result.json` files revealed the model is NOT looping (as turn 49 worried) — it's making different queries:

- **Round 05** `search_repo {"query": "compound_model6"}` → `grep_exit_code: 1` (no matches).
- **Round 06** `search_repo {"query": "compound_model9"}` → `grep_exit_code: 1` (no matches).

The task manifest's FAIL_TO_PASS list contains:
- `astropy/modeling/tests/test_separable.py::test_separable[compound_model6-result6]`
- `astropy/modeling/tests/test_separable.py::test_separable[compound_model9-result9]`

`compound_model6` and `compound_model9` are pytest parametrize labels (the bracketed `[...]` part is the parametrize identifier, conventionally generated from the parameter name + index). They are NOT codebase identifiers; `git grep` won't find them because they don't appear in source. They appear only in pytest's runtime-rendered test IDs.

The model interprets the FAIL_TO_PASS test names too literally and feeds the parametrize labels into `search_repo`. Two grep_exit_code-1 rounds wasted (~36 min total).

## Why this is harness-relevant (and generic)

Every SWE-bench task with parametrized failing tests will exhibit this pattern. The fix is generic:

- **Option A** (prompt-template): add a system-prompt directive explaining that pytest parametrize labels (the bracketed part of test IDs) are runtime-generated and NOT searchable. Steer the model toward searching the test fixture file, the parametrize decorator, or function names.
- **Option B** (harness pre-processing): in the problem-reality-packet builder, strip parametrize brackets from the FAIL_TO_PASS list before exposing it to the inner actor — OR add a parallel `fail_to_pass_search_terms` field with cleaned identifiers.
- **Option C** (do nothing): wait for the model to figure it out (round-07 advanced to `open_file_window`, suggesting it learned).

Option A is least invasive. Option B is more thorough but changes the packet schema. Option C accepts the ~36-min waste per attempt.

## Why I'm NOT acting on this in turn 50

- Single attempt's evidence. Need to see whether this pattern holds across multiple SWE-bench tasks (other tasks have different test naming conventions; some don't parametrize).
- Turn 47's wallclock budget already addresses the "wasted rounds" symptom. With 45-min budget at 18 min/round, the model gets ~2-3 rounds — including the wasted grep rounds, that means the wallclock-cap can prevent runaway exploration even without fixing the search-query specifically.
- More urgent priority is to LET this attempt finish (or be terminated) and produce an attempt-summary, then have a baseline.
- Single-writer cap: zero used this turn. If I dispatch the editor for a prompt-template tweak now, we'd need to wait for the next attempt to validate it — same wait as for the wallclock-cap, but with TWO untested changes. Better to ship one at a time.

## Source of truth

- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/agent-tools/round-05-00-search_repo/tool-result.json`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/agent-tools/round-06-00-search_repo/tool-result.json`
- `file:/home/cabdru/contextgraph/tasks/swebench-lite/astropy__astropy-12907.json` (the FAIL_TO_PASS list source)

## Handoff

Future agent: after the next clean attempt or two with the wallclock-cap, evaluate whether this parametrize-label-search pattern recurs. If it does on multiple tasks, ship Option A or B. If it's task-specific to astropy, leave it.
