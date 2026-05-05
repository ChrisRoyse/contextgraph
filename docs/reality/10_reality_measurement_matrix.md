# Reality Measurement Matrix

This file defines what must be measured so ContextGraph can optimize toward 100% SWE-bench smoke success.

## Measurement Principle

Every optimization needs a measured reality target.

If we cannot observe it, we cannot optimize it. If we cannot verify it from durable state, we cannot trust it.

## System-Level Measurements

| Area | What To Measure | Source Of Truth |
|---|---|---|
| Docker health | Docker CLI path, socket, engine version, container status | Docker Desktop socket and `docker version` |
| Disk health | free space before/after, run root size, Docker cache size | filesystem readback |
| Runtime budget | elapsed time per attempt, per official eval, total loop time | command logs and run summaries |
| GPU/model runtime | model loaded, prompt/eval tokens, latency, timeout, VRAM when available | Ollama response and `nvidia-smi` readback |
| Process integrity | exit codes, stdout/stderr hashes, timeout status | process result artifacts |

## Task Measurements

| Area | What To Measure | Source Of Truth |
|---|---|---|
| Dataset identity | dataset, split, instance id, repo, base commit | SWE-bench dataset row |
| Test targets | FAIL_TO_PASS and PASS_TO_PASS lists | SWE-bench dataset row |
| Problem statement | prompt hash and redacted preview | persisted task manifest |
| Gold/test patch hashes | oracle patch SHA, test patch SHA | persisted task manifest |

## Repository Measurements

| Area | What To Measure | Source Of Truth |
|---|---|---|
| Checkout | repo URL, base commit, HEAD SHA | git readback |
| Clean state | `git status --porcelain` before attempt | git readback |
| Candidate changes | changed files, diff SHA, hunk count | git diff |
| Patch applicability | `git apply` exit code and stderr | process artifact |
| Test patch applicability | official test patch apply result | process artifact |

## Model Measurements

| Area | What To Measure | Source Of Truth |
|---|---|---|
| Prompt | prompt SHA, prompt template id, included evidence refs | prompt manifest |
| Output | output SHA, parse status, patch hash | model response artifact |
| Tool/action | selected tool, arguments, schema validation | ContextGraph tool record |
| Agent tool transcript | read-only tool requests, outputs, file hashes, branch refs | `attempts/attempt-NNN/agent-tools/agent-tool-transcript.json` |
| Tool source readback | file search/read results and evidence previews | per-tool result JSON files with SHA-256 |
| Repetition | whether patch hash was seen before | attempt registry |
| Cross-model comparison | same task, same attempt budget, same prompt compiler version | experiment registry |

## Official SWE Measurements

| Area | What To Measure | Source Of Truth |
|---|---|---|
| Official status | resolved/unresolved/error | official final report |
| Stop condition | instance id in `resolved_ids` | official final report |
| Patch result | patch exists and applied | official instance report |
| FAIL_TO_PASS | success/failure tests | official instance report |
| PASS_TO_PASS | regression tests | official instance report |
| Test output | assertion excerpts and traceback | official `test_output.txt` |
| Early evaluator failure | patch-apply failure, missing instance report, missing test output | official readback + run-instance log |

## BlackBoxFix Measurements

| Area | What To Measure | Source Of Truth |
|---|---|---|
| Ledger rows | counts by record kind | SQLite `ledger_records` |
| Hash chain | ledger tip and record hashes | SQLite metadata and rows |
| Expected delta | expected persisted outcome | ledger payload |
| Actual readback | physical source-of-truth readback | ledger payload |
| Verifier result | passed/failed/blocked | ledger payload |
| Incident | incident count and remediation | ledger payload |
| Evidence packet | evidence refs and hashes | ledger payload |

## DynamicJEPA Measurements

| Area | What To Measure | Source Of Truth |
|---|---|---|
| State before action | repository/task/model state vector | DynamicJEPA state store |
| Predicted outcome | predicted verifier/reward/test outcome | DynamicJEPA prediction row |
| Actual outcome | official verifier/reward/test outcome | readback + ledger |
| Surprise | error norm, cosine, threshold, kind | DynamicJEPA surprise event |
| Plan trace | selected strategy and utility | DynamicJEPA plan trace |
| Training episode | state/action/outcome sample | exported episode file |

## Reward Measurements

| Area | What To Measure | Source Of Truth |
|---|---|---|
| Reward | raw, clipped, normalized reward | reward compilation row |
| Loss | negative reward for minimization | reward compilation row |
| Components | contribution per feature | reward compilation row |
| Spec integrity | reward spec SHA and version | reward row + spec file |
| Reward delta | attempt N vs N-1 | reward series |
| Champion reward | best verified attempt | champion state file |

## Prompt Compiler Measurements

| Area | What To Measure | Source Of Truth |
|---|---|---|
| Included evidence | which failure facts entered prompt | prompt manifest |
| Excluded evidence | which facts were omitted and why | prompt manifest |
| Token budget | prompt length and sections | prompt manifest |
| Evidence freshness | source run/attempt ids | prompt manifest |
| No-repeat constraints | patch hashes and strategies to avoid | prompt manifest |
| Prompt effectiveness | reward by prompt template version | scoreboard |

## Optimization Measurements

| Area | What To Measure | Source Of Truth |
|---|---|---|
| Solve rate | official resolved / total | official reports |
| Attempt efficiency | attempts to resolution | experiment registry |
| Repair rate | failed attempt followed by improved attempt | reward series |
| Repeated failure rate | same error/patch repeating | failure clusters |
| Cross-model transfer | improvement learned from one model helps another | scoreboard |
| Overfit gap | dev_smoke vs holdout_smoke | split reports |
| Cost | wall time, Docker time, GPU time | run summaries |

## Minimum Data Needed For The 100% Goal

To make the 100% target possible, every attempt must capture:

1. full task identity;
2. exact prompt hash and prompt manifest;
3. exact model output hash;
4. candidate patch hash and diff;
5. official Docker report;
6. official instance report;
7. official test output excerpt;
8. BlackBoxFix verifier/evidence/incident rows;
9. reward/loss with components;
10. next prompt evidence manifest;
11. repeated patch/failure history;
12. champion state;
13. tool transcript and tool result hashes;
14. branch lineage for attempt and prompt-candidate realities.

Without these, the loop cannot know what reality changed, what failed, or what to improve next.
