# Meta Reality Loop

## Intention

We need two loops, not one.

```text
Outer loop: Reality Loop Builder
  creates and improves
Inner loop: SWE Reality Solver
  solves SWE-bench tasks using verified reality
```

The outer loop's purpose is to build the best possible inner loop. The inner loop's purpose is to get all three local models to solve the selected SWE-bench suite with official source-of-truth proof.

## Explicit Goal

The working goal is:

> Build a ContextGraph-controlled reality loop that can drive `qwen3.6:27b` toward 100% official success on a smoke SWE-bench Lite suite, then expand only when the loop proves improvement without reward hacking. (Earlier project drafts envisioned multiple local models in parallel; that scope was deliberately reduced to a single model so optimizer effort stays focused on harness quality.)

This is an optimization target, not a claim of current capability. The system can only claim 100% after the official SWE-bench report proves every selected instance is in `resolved_ids` for every selected model.

## The Two Loops

### Inner Loop: SWE Reality Solver

The inner loop does this:

```text
task -> model prompt -> candidate patch -> official Docker eval
  -> source-of-truth readback -> reward/loss
  -> compact failure evidence -> next prompt
```

It is model-facing. It asks a model to repair a real SWE-bench issue.

### Outer Loop: Reality Loop Builder

The outer loop does this:

```text
inner-loop run results -> diagnose loop bottleneck
  -> modify one loop artifact
  -> rerun smoke suite
  -> compare verified scoreboard
  -> keep/discard loop change
```

It is system-facing. It improves the machinery around the model.

## Multiple Realities

Multiple realities are possible and useful when they are represented as explicit branches, not as untracked speculative text.

In the current implementation:

```text
root reality
  -> model/task reality
    -> attempt reality
      -> prompt-candidate reality
```

Each branch has:

- `reality_branch_id`;
- `parent_reality_branch_id`;
- `promotion_status`;
- source-of-truth refs;
- SHA-256 evidence where applicable.

The registry is persisted at:

```text
<run_root>/reality-scoreboard/reality-branches.json
```

This lets the system explore many candidate next prompts, tools, context selectors, or repair strategies while keeping only verified branches eligible for promotion. A branch is not "true" because the model prefers it. It becomes promotable only when the official source of truth and BlackBoxFix ledger prove the outcome.

## What The Outer Loop Is Optimizing

The outer loop optimizes:

- prompt compiler quality;
- repository context selection;
- failure evidence compression;
- tool schema clarity;
- Agent-Computer Interface design;
- retry policy;
- no-repeat patch policy;
- cross-model memory;
- task curriculum;
- reward spec candidates in shadow mode;
- training/export data quality;
- LoRA/fine-tune recipe candidates;
- dashboard/observability clarity.

It does not optimize by changing the benchmark answer key.

## Immutable Reality Anchors

The outer loop cannot change:

- SWE-bench dataset rows;
- base commits;
- official test patches;
- official Docker evaluator;
- official `resolved_ids` stop condition;
- production verifier definitions;
- production reward spec without review;
- ledger hash chain;
- source-of-truth connectors;
- held-out task set;
- audit records.

These are the rails that prevent the system from optimizing into fraud.

## Outer Loop Keep/Discard Rule

A proposed loop change is kept only if it improves a verified scoreboard without breaking governance:

```text
keep if:
  official_resolved_count improves
  OR reward improves with no loss in source-of-truth coverage
  OR failure evidence quality improves and repeated failure decreases

discard if:
  success does not improve
  OR incidents increase without useful diagnostic gain
  OR source-of-truth coverage drops
  OR prompt size grows with no measured benefit
  OR verifier/policy integrity changes outside approved scope
```

## Why This Is Different From A Normal Agent Loop

Normal self-reflection loop:

```text
model fails -> model critiques itself -> model retries
```

ContextGraph meta reality loop:

```text
model fails -> official reality proves why
  -> ledger captures evidence
  -> reward decomposes failure
  -> outer loop changes the system artifact most likely responsible
  -> inner loop retries under measured conditions
```

The learning signal is reality, not confidence.

## Tool Boundary For The Inner Loop

The model is not given arbitrary shell or browser control by default. It receives a governed tool surface:

- `list_files`;
- `read_file`;
- `search_repo`;
- `inspect_repository_context`;
- `read_failure_evidence`.

Every tool call is validated, executed as read-only, written to disk, read back, and referenced by SHA. Mutating actions remain constrained to final patch submission and are verified by local patch application plus official SWE-bench Docker evaluation.

Browser/MCP/web tools can be added later, but only as registered ContextGraph tools with schemas, policy, source-of-truth readback, and ledger evidence. They should not bypass the reality loop.
