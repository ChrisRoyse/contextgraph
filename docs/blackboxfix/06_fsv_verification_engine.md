# FSV Verification Engine

The verifier is the core product. It converts "the agent said it worked" into
"the source of truth proves it worked" — or a typed failure.

Phase 1 shipped a structural verifier for the ledger (hash-chain integrity
across the SQLite source of truth). Phase 2 ships the first outcome verifiers
for local Git and command artifacts. Phase 3 ships GitHub/CI.

## Full-State Verification Protocol

Every mutating action follows this protocol — both at runtime (the verifier)
and during manual testing (the operator). The full operator-side protocol
lives in `19_full_state_verification_protocol.md`.

1. Define the source of truth.
2. Print and persist the before-state.
3. Execute the trigger action.
4. Read the source of truth through an **independent** read path.
5. Decode and normalize the observed state.
6. Compare expected persisted delta to actual readback.
7. Emit `VerifierResult`.
8. Persist an `EvidencePacket`.
9. Export the action as training/eval data only with verifier status attached.

## Source Of Truth

The source of truth is the durable system where the action's final state
lives. Examples:

- Git object database for commits;
- GitHub API for pull request state;
- CI provider API for workflow status;
- database tables for application records;
- Kubernetes API for cluster objects;
- Terraform state plus cloud API for infrastructure;
- ticketing API for issue state.

For Phase 1 the durable system is the SQLite `ledger_records` table itself.
A verifier of the ledger reads back the rows and recomputes both hash chains.

Return values, logs, stdout, model text, and tool summaries are evidence but
**not** the source of truth unless the durable artifact actually is that
log/file.

## Verifier Registry

Shipped Phase 2 verifier ids:

| Verifier | Connector | Source of truth | Main comparison |
|---|---|---|---|
| `git_branch_exists_v1` | `git_local_v1` | `refs/heads/<branch>` | branch ref resolves to a commit |
| `git_commit_exists_v1` | `git_local_v1` | Git object DB | commit-ish resolves to a commit and object type is recorded |
| `git_file_diff_matches_patch_v1` | `git_local_v1` | `git diff --no-ext-diff --no-color -- <path>` | exact patch string/hash match |
| `git_working_tree_clean_v1` | `git_local_v1` | `git status --porcelain=v1 --untracked-files=all` | clean bool equals expected |
| `cmd_exit_zero_v1` | `cmd_local_v1` | process exit/stdout/stderr + declared artifact files | exit code zero and artifacts exist |

All verifier specs are JSON objects containing `verifier_id`, `run_id`,
`tool_call_id`, `source`, and `expected`. Schema/type failures are `blocked`;
readback mismatches are `failed`; matches are `passed`.

## Future Verifier Registry Shape

Verifiers are versioned specs. JSON shape:

```json
{
  "verifier_id": "github_pr_open_v1",
  "source_kind": "github",
  "input_schema": "schemas/github_pr_open_input_v1.json",
  "expected_delta_schema": "schemas/github_pr_open_delta_v1.json",
  "readback_schema": "schemas/github_pr_readback_v1.json",
  "comparison": "exact_fields",
  "required_fields": ["repo", "pr_number", "head_ref", "state"],
  "edge_cases": ["missing_pr", "wrong_head_ref", "closed_pr"]
}
```

Versioning: `verifier_id` ends in `_v<N>`; bumping schema bumps the suffix.
No backwards compatibility — older specs are removed, not aliased.

## Status Semantics

| Status | Meaning | Phase 1 example |
|---|---|---|
| `passed` | readback proves the expected delta | `inspect ledger` returned `integrity.status=passed` |
| `failed` | readback completed and contradicts the expected delta | tampered payload caused `BBX_HASH_CHAIN_BROKEN` |
| `blocked` | verifier cannot run because a required precondition failed | ledger missing → `BBX_LEDGER_NOT_INITIALIZED` |
| `not_applicable` | action is read-only or intentionally has no persisted delta | n/a in Phase 1 |

`blocked` must include a concrete error code:

- `CONNECTOR_PERMISSION_DENIED`
- `SOURCE_OF_TRUTH_UNAVAILABLE`
- `READBACK_SCHEMA_DRIFT`
- `VERIFIER_SPEC_MISSING`
- `REDACTION_POLICY_VIOLATION` (Phase 1: surfaces as `BBX_REDACTION_VIOLATION`)
- `EVIDENCE_WRITE_FAILED`

## Comparison Modes

| Mode | Definition |
|---|---|
| **Exact fields** | expected value must equal readback value (string/number/bool/JSON-equal) |
| **Set membership** | expected item must exist in source-of-truth collection |
| **Count delta** | `after_count - before_count == expected_delta` |
| **Hash match** | artifact hash equals expected hash |
| **Predicate** | verifier-defined predicate over normalized readback |
| **Temporal invariant** | a condition must hold within a bounded window; **all** polling attempts recorded |

## Edge-Case Requirement

Every mutating verifier must define and test at least three edge cases. Example
for `github_pr_open_v1`:

- PR missing → `GITHUB_PR_NOT_FOUND`.
- Wrong branch → `GITHUB_PR_HEAD_REF_MISMATCH`.
- Closed PR → `GITHUB_PR_STATE_MISMATCH`.

For each edge case the evidence must show before-state, action attempted,
readback state, typed error, and remediation.

Phase 1 already follows this rule for the ledger verifier:

| Edge case | Synthetic input | Expected error code | Test |
|---|---|---|---|
| Redaction violation | tool input contains `Bearer should-not-be-persisted` | `BBX_REDACTION_VIOLATION` (exit 1) | `phase1_redaction_violation_blocks_persistence` |
| Hash chain tampering | privileged update of `payload_json` after dropping the no-update trigger | `BBX_HASH_CHAIN_BROKEN` (exit 2) | `phase1_hash_chain_detection_catches_payload_tampering` |
| Append-only trigger bypass | direct `UPDATE ledger_records SET record_kind=...` | SQLite `RAISE(ABORT)` (exit non-zero) | `phase1_append_only_trigger_blocks_direct_update` |

Phase 2 follows the same rule in
`crates/context-graph-cli/tests/blackbox_phase2_fsv.rs`: each shipped mutating
verifier has real-source happy-path coverage plus missing target, wrong target,
and schema/connector failure coverage.

## Evidence Packet (Phase 4 target)

```json
{
  "evidence_packet_id": "ev_...",
  "run_id": "run_...",
  "tool_call_id": "tool_...",
  "verifier_result_id": "verify_...",
  "source_of_truth": [{
    "kind": "github",
    "locator": "github:org/service/pull/123",
    "read_at": "RFC3339",
    "state_hash": "sha256:..."
  }],
  "before_state_hash": "sha256:...",
  "after_state_hash": "sha256:...",
  "verdict": "passed"
}
```

## Robust Logging

Every verifier failure must answer:

- Which source of truth failed?
- Which connector operation failed?
- Which field was expected?
- What did readback observe?
- Was this a policy block, execution failure, or outcome mismatch?
- What action should a human or agent take next?

Phase 2 enforces this with the `BlackboxError` JSON shape (`status`,
`error_code`, `message`, `source_of_truth`, `connector`, `field_path`,
`remediation`).

## Training Signal

Verifier output becomes supervision (Phase 5):

- `passed` examples → correct tool use.
- `failed` examples → outcome mismatch detection.
- `blocked` examples → missing preconditions / governance limits.
- `remediation` fields → recovery actions.

## FSV Of This Document

The Phase 1/2 verifiers are runnable demonstrations. To prove the doc:

```bash
ledger=/tmp/bbx-fsv.sqlite
context-graph blackbox init --ledger "$ledger"
context-graph blackbox record run --ledger "$ledger" --task happy --agent a
context-graph blackbox inspect ledger --ledger "$ledger" --run-id <RUN>
# integrity.status must be "passed", record_count must be 1.

# Edge case: corrupt the chain
sqlite3 "$ledger" "DROP TRIGGER ledger_records_no_update; UPDATE ledger_records SET payload_json='{}' WHERE sequence=1;"
context-graph blackbox inspect ledger --ledger "$ledger"
# Must exit 2 with error_code BBX_HASH_CHAIN_BROKEN.

# Phase 2 happy path uses a real Git repo and command artifact; see
# tests/blackbox_phase2_fsv.rs and docs/blackboxfix/21_phase2_state_and_invariants.md.
```

If those two outputs do not match, this doc is wrong; update it.
