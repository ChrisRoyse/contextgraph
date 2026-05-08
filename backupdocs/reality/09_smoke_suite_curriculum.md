# Smoke Suite Curriculum

## Purpose

The smoke suite must be hard enough that models fail in different ways, but small enough that we can run it repeatedly while developing the loop.

If every model gets 100% immediately, the suite is useless for optimization. If every model fails with opaque environment errors, the suite is also useless. The ideal smoke suite produces measurable partial progress.

## Initial Suite Size

Start with:

```text
Stage 0:
  1 SWE-bench Lite instance
  3 models
  2 attempts per model
  official Docker eval after each attempt
```

This gives:

```text
1 task * 3 models * 2 attempts = 6 official evaluations
```

That is enough to prove:

- the inner loop retries;
- official failure evidence reaches the next prompt;
- different models produce different patch/reward trajectories;
- repeated patch detection works;
- the outer loop has a scoreboard to optimize.

## Current Seed Task

Use `psf__requests-2317` as the first task because it already produced a useful signal:

- Qwen generated a patch that applied;
- official Docker eval remained unresolved;
- official FAIL_TO_PASS had 7 successes and 1 failure;
- failure target: `test_requests.py::TestTimeout::test_encoded_methods`;
- full official output also exposed `test_conflicting_post_params`.

That is ideal for a first reality loop because the failure is not just "patch invalid." It gives the next prompt a real target.

## Stage 1

Add the existing second smoke task:

```text
Stage 1:
  2 SWE-bench Lite instances
  3 models
  2 attempts per model
```

This gives 12 official evaluations. It tests whether the loop generalizes beyond one repo.

Use:

- `psf__requests-2317`
- `django__django-12908`

## Stage 2

Expand to:

```text
Stage 2:
  5 SWE-bench Lite instances
  3 models
  3 attempts per model
```

This gives 45 official evaluations.

Add tasks only if:

- prior stage completed with complete source-of-truth readbacks;
- Docker cache/disk health was inspected;
- at least one failure became a better attempt;
- no success was claimed without official `resolved_ids`;
- repeated patches are detected and penalized.

## Stage 3

Use:

```text
Stage 3:
  10 SWE-bench Lite instances
  3 models
  3-5 attempts per model
```

This becomes the first meaningful local benchmark suite.

## Selection Criteria

Pick tasks that:

- have non-empty `FAIL_TO_PASS`;
- run inside official Docker harness;
- are not all trivially solved by all three models;
- produce actionable failure logs;
- represent multiple repositories;
- have moderate runtime;
- do not require huge external downloads beyond official harness images;
- expose different failure modes.

Avoid tasks that:

- fail due to Docker infrastructure;
- depend on unavailable credentials;
- are extremely slow;
- are already solved by every model on first attempt;
- require broad multi-file architectural rewrites before the loop is stable.

## Split Design

Use three splits from the beginning:

| Split | Purpose |
|---|---|
| `dev_smoke` | Used by the outer loop to optimize artifacts. |
| `holdout_smoke` | Used to check whether improvements generalize. |
| `promotion_smoke` | Used only before claiming a new best loop. |

Never optimize directly against the promotion split.

## Success Criteria For Smoke Development

Before expanding beyond Stage 1, the system must prove:

- official Docker readback is present for every attempt;
- attempt N failure appears in attempt N+1 prompt;
- BlackBoxFix ledger has official verifier rows;
- reward changes across attempts;
- prompt compiler changes are traceable;
- no repeated identical patch is treated as meaningful progress;
- every model has its own attempt/reward trajectory.
