# Measurement Contract

The measurement-spec boundary is now a runtime contract, not a narrative assumption.

## Problem

If ContextGraph measures exactly the wrong reality, the system can be deterministic and still produce a misleading conclusion. The model cannot directly hack the reward path, but the system can still fail if the oracle, source of truth, or claim scope is incomplete.

## Runtime Contract

The Rust SWE reality loop now writes:

```text
<run_root>/measurement-contract.json
```

The contract records:

- the exact claim scope;
- the success predicate;
- explicit non-claims;
- official SWE-bench dataset, split, and instance id;
- FAIL_TO_PASS and PASS_TO_PASS coverage metadata;
- immutable boundaries;
- mandatory readbacks;
- model-editable scope;
- model-forbidden scope;
- fail-closed policy.

For SWE-bench, the only success claim is:

```text
official_report.resolved_ids contains task.instance_id
```

Local patch application is useful evidence, but it is not allowed to become a success claim.

## Fail-Closed Rules

The loop now errors before execution if:

- `--official-eval-each-attempt` is missing;
- the task manifest dataset is not `princeton-nlp/SWE-bench_Lite`;
- the instance id is malformed;
- the repository is not an owner/repo identifier;
- the base commit is not a full 40-character git SHA;
- the problem statement is empty;
- FAIL_TO_PASS tests are missing;
- PASS_TO_PASS metadata is missing;
- the task source of truth does not bind dataset and instance id.

The workspace reset also verifies:

- `git rev-parse HEAD` equals the manifest base commit;
- `git status --porcelain` is empty before an attempt starts.

Official evidence now verifies that the target instance is classified by the official report, not merely that a report file exists.

## Source-Of-Truth Readbacks

The current mandatory source-of-truth readbacks are:

- task manifest file readback;
- measurement contract file readback;
- git HEAD readback;
- git clean-state readback;
- candidate patch application readback;
- changed-file readback with test/harness mutation blocking;
- Docker SDK preflight through Docker Desktop's WSL socket;
- official SWE-bench final report readback;
- BlackBoxFix SQLite ledger readback;
- append-only event log readback.

## Research Basis

- SWE-bench evaluates predictions by applying patches to real repositories and running tests in Docker; it reports completed and resolved instances: https://www.swebench.com/SWE-bench/guides/evaluation/
- W3C PROV frames provenance around entities, activities, and responsible agents, which maps directly to prompt, patch, evaluator run, report, and ledger rows: https://www.w3.org/TR/2012/WD-prov-dm-20120503/
- NIST AI RMF emphasizes documented measurement and evaluation as part of AI risk management: https://airc.nist.gov/airmf-resources/
- The test-oracle problem makes the boundary explicit: tests are oracles for defined claims, not proof of all possible intent: https://en.wikipedia.org/wiki/Test_oracle

## Current Manual Verification

Edge cases intentionally failed closed and wrote durable failure artifacts:

| Case | Expected | Observed Source Of Truth |
|---|---|---|
| Official eval omitted | error before execution | `/tmp/contextgraph-boundary-edge-official-disabled/failures/failure-1777962519-RUST_REALITY_OFFICIAL_EVAL_REQUIRED.json` |
| Invalid attempt budget | error before execution | `/tmp/contextgraph-boundary-edge-invalid-budget/failures/failure-1777962527-RUST_REALITY_MAX_ATTEMPTS_INVALID.json` |
| Bad dataset manifest | error before execution | `/tmp/contextgraph-boundary-edge-bad-manifest/failures/failure-1777962530-RUST_REALITY_TASK_DATASET_MISMATCH.json` |

Happy-path official Docker readback completed:

```text
/tmp/contextgraph-official-report-cwd-smoke/gold.rust_contract_gold_psf_requests_2317.json
```

Observed:

```json
{
  "total_instances": 1,
  "completed_instances": 1,
  "resolved_instances": 1,
  "completed_ids": ["psf__requests-2317"],
  "resolved_ids": ["psf__requests-2317"]
}
```

This proves the official evaluator can produce the durable report that the Rust loop now treats as the only SWE success oracle.
