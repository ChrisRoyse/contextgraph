# Prompt Reinjection Protocol

The inner loop can improve only if verified failure evidence becomes useful next-attempt context.

## Principle

Reinject facts, not logs.

The model should receive compact verified facts from the source of truth, with enough detail to repair the issue and enough constraints to avoid repeating the same failed action.

## Inputs

For attempt N+1, use:

- original problem statement;
- exact repo and base commit;
- previous prompt SHA;
- previous output SHA;
- previous patch SHA;
- previous patch diff;
- official resolved/unresolved/error status;
- failed FAIL_TO_PASS tests;
- failed PASS_TO_PASS regressions;
- assertion/traceback excerpts;
- files changed by previous patch;
- relevant source snippets re-read from base checkout;
- incident summary;
- reward components;
- no-repeat constraints.

## Exclude

Do not include:

- unverified model explanations as facts;
- unrelated full test logs;
- raw secrets;
- entire repository dumps;
- stale evidence from older attempts when newer readback supersedes it;
- hidden gold patch;
- official test patch content as a repair target if that would leak benchmark answer structure beyond allowed test context.

## Prompt Structure

```text
1. Task identity
2. Success condition
3. Problem statement
4. Previous attempt summary
5. Verified official failure evidence
6. Relevant source snippets
7. Previous patch summary and no-repeat constraints
8. Required output schema
```

## Required Success Condition Text

Every SWE prompt should state:

```text
Success means the official SWE-bench Docker report puts this instance id in resolved_ids.
Local patch application or model confidence is not success.
```

## Failure Evidence JSON

Persist:

```json
{
  "evidence_kind": "official_swe_failure_for_reprompt",
  "source_attempt": 1,
  "official_status": "failed",
  "resolved": false,
  "patch_successfully_applied": true,
  "failed_fail_to_pass": [
    "test_requests.py::TestTimeout::test_encoded_methods"
  ],
  "failed_pass_to_pass": [],
  "test_output_excerpt": "FAILED ... assert False",
  "previous_patch_sha256": "sha256:...",
  "reward": -3.515,
  "reward_components": [],
  "source_of_truth": {
    "official_report": "file:...",
    "instance_report": "file:...",
    "test_output": "file:...",
    "ledger": "sqlite:..."
  }
}
```

## Prompt Manifest

For every prompt, persist:

```json
{
  "prompt_sha256": "sha256:...",
  "prompt_template_id": "swe_repair_official_failure_v1",
  "attempt": 2,
  "included_evidence": [
    {
      "kind": "official_failed_test",
      "value": "test_requests.py::TestTimeout::test_encoded_methods",
      "source_of_truth": "file:..."
    }
  ],
  "excluded_evidence": [
    {
      "kind": "full_test_output",
      "reason": "too large; compact excerpt included instead"
    }
  ],
  "no_repeat_patch_sha256": ["sha256:..."]
}
```

## Reinjection Quality Score

Score every prompt:

| Feature | Meaning |
|---|---|
| `failed_test_included` | The official failed test appears in prompt. |
| `assertion_excerpt_included` | A useful error excerpt appears. |
| `source_snippet_included` | Relevant code was re-read and included. |
| `previous_patch_hash_included` | The prior patch is traceable. |
| `no_repeat_constraint_included` | The model is explicitly told not to repeat unchanged patch. |
| `hidden_solution_leak_absent` | Gold patch and answer leakage are absent. |

This lets the outer loop optimize the prompt compiler itself.
