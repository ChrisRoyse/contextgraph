# Grand-Scale Reality AutoResearch

This project should use AutoResearch as an inspiration pattern, not as a direct copy.

Karpathy's AutoResearch loop is deliberately small: an agent reads `program.md`, edits `train.py`, runs a fixed-budget experiment, measures `val_bpb`, and keeps only improvements. ContextGraph needs the same core discipline, but at a higher level:

```text
outer OpenAI coding agent
  -> edits the ContextGraph reality-loop system
  -> verifies project reality with Rust/build/git readbacks
  -> improves the inner Gemma SWE loop

inner Gemma SWE agent
  -> attempts real SWE-bench repair through governed tools
  -> receives problem/failure/reward reality
  -> stops only when official SWE-bench durable readback proves success
```

## Generalized Pattern

The useful AutoResearch ideas are:

- write an explicit research protocol;
- let the agent edit real code;
- run bounded experiments;
- measure durable outcomes;
- keep history;
- reject changes that fail the real metric.

ContextGraph generalizes those ideas:

- the editable code is the reality-loop harness, prompt compiler, reward-signal compiler, and UI/runtime surfaces;
- the neutral judge is not one scalar `val_bpb`, but a set of source-of-truth readbacks;
- the inner success predicate is official SWE-bench `resolved_ids`;
- the outer success predicate is project verification plus improved inner-loop behavior;
- every agent gets its own reality packet.

## Reality Packets

Gemma receives:

- `problem-reality-packet.json`;
- `reality-signal-packet.json`;
- official SWE-bench failure/success evidence;
- governed tool transcripts;
- reward and score realities.

The OpenAI outer coding agent receives:

- Gemma outcome reality;
- its own tool-call reality;
- its own project-edit readbacks;
- git status and git diff reality;
- `cargo check` / test / build reality;
- `openai-self-reality-packet.json`.

The two agents should not receive the same raw blob. Each receives the reality required for its job.

## Accept / Reject Rules

Gemma changes are accepted only if:

- the candidate patch was produced by a governed edit tool;
- the patch artifact was read back from disk;
- official SWE-bench evaluation runs;
- the official report places the instance in `resolved_ids`.

OpenAI outer-loop changes are accepted only if:

- edits target governed ContextGraph project paths;
- every edit uses an observed file SHA-256;
- file readbacks prove the edit landed;
- git diff/status prove the project state;
- Rust formatting and `cargo check` pass;
- no benchmark tests, verifier trust boundaries, or secrets were touched.

## Current Implementation

The Rust loop implements this in `crates/context-graph-cli/src/bin/reality-loop.rs`.

Outer-loop tools:

- `project_search`;
- `project_open_file_window`;
- `project_apply_line_window_edit`;
- `project_run_command`;
- reality readers for Gemma attempt artifacts.

Outer-loop self-reality:

```text
<attempt_root>/openai-loop-optimizer/openai-self-reality-packet.json
```

That packet is fed back into the final optimizer step so the model sees the reality of its own actions before producing guidance for the next loop iteration.

## Research Basis

- Karpathy AutoResearch repository: https://github.com/karpathy/autoresearch
- AutoResearch program contract: https://github.com/karpathy/autoresearch/blob/master/program.md
- OpenAI function calling guide: https://platform.openai.com/docs/guides/function-calling
- OpenAI Responses API reference: https://platform.openai.com/docs/api-reference/responses
- OpenAI structured outputs guide: https://platform.openai.com/docs/guides/structured-outputs
