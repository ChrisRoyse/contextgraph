# Path To 100 Percent SWE Smoke Success

## Target

The target is:

```text
For each selected smoke SWE-bench task:
  gemma4:e4b resolves it
  qwen3:8b resolves it
  mistral-nemo:12b resolves it

Proof:
  official SWE-bench report contains every instance id in resolved_ids
  for every model run.
```

This document defines a strategy. It is not a claim that the current system has achieved it.

## Why 100 Percent Is Plausible On A Smoke Suite

It may be possible on a small, carefully selected suite because ContextGraph can provide:

- exact source-of-truth failure evidence;
- repeated attempts;
- cross-model transfer;
- prompt compiler optimization;
- repository context optimization;
- verified reward/loss;
- training examples from failed and successful attempts;
- LoRA/fine-tuning candidates;
- official Docker feedback.

The smaller the suite, the more likely 100% becomes. The hard part is preventing overfit and false claims.

## Optimization Layers

### Layer 1: Inference-Time Refinement

Use official failure evidence to reprompt the same model.

Expected gains:

- patch apply rate improves;
- repeated errors decrease;
- failed test count decreases.

### Layer 2: Prompt Compiler Optimization

Outer loop modifies prompt structure and evidence selection.

Expected gains:

- model pays attention to right failed test;
- fewer malformed diffs;
- fewer irrelevant edits.

### Layer 3: Agent-Computer Interface Optimization

Improve tools and observations:

- line-numbered source snippets;
- search result summaries;
- patch validation before official eval;
- explicit changed-file summaries;
- compact official failure cards.

Expected gains:

- better localization;
- less hallucinated code;
- lower token waste.

### Layer 4: Cross-Model Transfer

Use one model's verified failure/success to help another model.

Examples:

- Gemma fails to locate file; Qwen found relevant file.
- Mistral produces malformed patch; Gemma produces valid diff pattern.
- Qwen resolves 7/8 tests; another model proposes final fix.

The system can generate cross-model hints only from verified evidence.

### Layer 5: Training Data Export

Create:

- positive samples from official resolved attempts;
- negative samples from official unresolved attempts;
- preference pairs from higher/lower reward attempts;
- rejected patch samples from malformed/repeated patches.

Then train:

- LoRA per model;
- shared prompt adapter;
- repository localization helper;
- patch-format correction model.

### Layer 6: Curriculum

Start from tasks where models make partial progress, then gradually expand.

Do not train only on solved tasks. The valuable data is the gap between almost-solved and solved.

## Required Score Movement

For each model/task pair, track:

```text
attempt 1:
  patch_applies?
  official_resolved?
  failed_fail_to_pass_count
  reward

attempt 2:
  patch_applies?
  official_resolved?
  failed_fail_to_pass_count
  reward

delta:
  did reality improve?
```

The loop is working if any of these improve:

- patch becomes parseable;
- patch starts applying;
- official error becomes official unresolved;
- failed test count drops;
- PASS_TO_PASS regressions drop;
- reward increases;
- repeated patch rate drops;
- final resolved status appears.

## Anti-Overfit Rule

The system can aim for 100% on the dev smoke suite, but cannot claim robust SWE-bench capability until:

- holdout smoke improves;
- promotion smoke passes;
- official evaluator remains untouched;
- reward spec remains approved;
- no hidden gold patch leakage exists.

## Three-Model Strategy

Run all three models independently first.

Then add shared memory:

```text
shared verified memory:
  failed tests by task
  relevant files by task
  patch hashes tried
  strategies that improved reward
  strategies that caused regressions
```

Each model receives:

- its own prior attempts;
- shared verified failure facts;
- no raw unverified claims from other models;
- no gold patch.

## What 100 Percent Looks Like In Evidence

For a two-task smoke suite and three models:

```text
6 model-task pairs
6 official reports
6 resolved entries
0 unresolved entries
0 error entries
complete ledger evidence for every pair
reward above promotion threshold for every pair
```

The final claim must point to the exact official report files and BlackBoxFix ledger rows.
