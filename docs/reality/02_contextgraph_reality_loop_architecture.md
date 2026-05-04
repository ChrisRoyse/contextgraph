# ContextGraph Reality Loop Architecture

## Product Category

**Reality-Verified Self-Optimization** is an agent operations and training architecture where every attempt is judged against durable state, not model self-assessment.

The system optimizes agents by repeatedly answering:

1. What did the model intend?
2. What action did ContextGraph allow?
3. What changed in the real system?
4. Did the source of truth match the expected delta?
5. What reward/loss does that reality imply?
6. What compact evidence should the next attempt receive?
7. Should this attempt become training data, an eval, an incident, or a promotion candidate?

## System Flow

```text
Operator objective
  -> model_io record
  -> LLM proposal/tool call
  -> ContextGraph action gateway
  -> policy decision
  -> real tool execution
  -> DynamicJEPA state/action/outcome/surprise
  -> BlackBoxFix expected delta
  -> independent source-of-truth readback
  -> verifier result
  -> evidence packet / incident packet
  -> scorecard
  -> reward compilation
  -> prompt compiler
  -> next attempt
```

## Required Subsystems

### 1. Model Runtime Adapter

Responsible for:

- model selection;
- prompt hashing;
- redacted prompt/output previews;
- token and latency capture;
- tool-call capture;
- model parameters;
- raw-content pointer when governance permits.

It must never be the source of truth for success.

### 2. Agent-Computer Interface

The interface should expose narrow ContextGraph tools rather than raw shell access.

Example SWE tool surface:

- `inspect_repository_context`
- `apply_candidate_patch`
- `run_local_validation`
- `run_official_swebench_eval`
- `read_official_failure_evidence`
- `compile_repair_prompt`
- `submit_candidate_patch`

Each tool requires:

- input schema;
- policy scope;
- expected persisted delta;
- executor;
- verifier;
- source-of-truth path;
- typed error codes.

### 3. Policy Gate

Before mutation:

- validate capability id;
- validate path/tool scope;
- validate risk tier;
- validate verifier coverage;
- persist `policy_decision`;
- fail closed if policy cannot be evaluated.

### 4. Execution Layer

Executes the allowed action against a real source:

- git checkout;
- file edit;
- test command;
- Docker eval;
- SQLite write;
- RocksDB/DynamicJEPA state write;
- ticket/API update;
- artifact export.

No execution result is considered final until the verifier reads durable state.

### 5. Source-of-Truth Verification

The verifier reads the durable system independently:

- file bytes and SHA-256;
- git diff and commit state;
- Docker official report;
- SQLite ledger rows;
- RocksDB counts;
- test output files;
- signed artifact manifest;
- external API state.

The source-of-truth record must include both:

```json
{
  "expected_persisted_delta": {},
  "actual_readback": {}
}
```

### 6. DynamicJEPA Memory

DynamicJEPA stores operational state transitions:

- state before action;
- action;
- predicted outcome;
- actual outcome;
- surprise;
- training episode;
- plan trace;
- provenance.

This lets ContextGraph optimize not only pass/fail, but also prediction accuracy and surprise reduction.

### 7. BlackBoxFix Ledger

BlackBoxFix is the tamper-evident evidence layer:

- ledger records;
- hash chain;
- model decisions;
- tool calls;
- expected deltas;
- readbacks;
- verifier results;
- evidence packets;
- incidents;
- scorecards;
- reward compilations.

### 8. Reward Compiler

The reward compiler maps verified reality into a scalar.

Current demo reward components:

- correctness;
- verification confidence;
- risk;
- incident presence;
- missing policy;
- training value.

In this project, `loss = -reward` is correct when the downstream optimizer minimizes loss while the system maximizes reward.

### 9. Prompt Compiler

The prompt compiler turns evidence into the next model input. It should include:

- previous prompt hash;
- previous output hash;
- patch diff hash;
- exact failed verifier;
- failed tests;
- relevant file snippets;
- incident summary;
- reward delta;
- banned repeated actions;
- current champion state.

It should exclude:

- raw secrets;
- irrelevant full logs;
- unverified model claims;
- stale evidence superseded by newer source-of-truth readbacks.

### 10. Promotion Gate

A candidate only advances when:

- all mandatory verifiers pass;
- no blocking policy decisions exist;
- reward improves over the current champion;
- source-of-truth coverage is complete;
- artifacts are signed or hash-anchored;
- held-out evals pass when required.

## Stop Conditions

The loop stops for success only when the durable source of truth says success.

For SWE-bench:

```text
success = instance_id in official_report.resolved_ids
```

For product demos:

```text
success = verifier_status == passed
       && evidence_hash_verification == passed
       && policy_status in allowed modes
       && reward >= promotion_threshold
```

For failures:

- blocked source-of-truth access;
- invalid policy;
- invalid verifier;
- missing credential;
- tampered ledger;
- no improvement after budget;
- repeated unchanged patches;
- unsafe action request.

All failures must persist an error record before exit.
