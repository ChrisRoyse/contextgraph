# Observability And Failure Policy

DynamicJEPA must fail loudly and leave enough evidence to fix the real cause. It must not convert broken behavior into passing tests or plausible-looking rows.

## Structured Logging

Use `tracing` with structured fields.

Every operation logs:

- `operation`;
- `run_id`;
- `domain_pack_id`;
- `source_of_truth_cf`;
- `target_id`;
- `status`;
- `elapsed_ms`;
- `record_count_before`;
- `record_count_after`;
- `error_code`;
- `error_message`;
- `remediation_hint`.

Errors must include concrete ids, not only generic messages.

## Run Records

Long-running or multi-stage processes need persisted run records:

- adapter run;
- dataset compile run if compilation spans multiple shards;
- training run;
- prediction run if inference becomes asynchronous;
- plan trace;
- verification run.

Run records make failures inspectable after a process exits.

## Error Taxonomy

Use typed errors across core, storage, service, MCP, and CLI.

Categories:

- `validation`: bad input or schema mismatch before persistence;
- `source_of_truth_missing`: expected persisted row missing;
- `storage`: RocksDB open/read/write/deserialize problem;
- `codec`: version mismatch, empty bytes, bincode failure, hash mismatch;
- `instrument`: required instrument failure;
- `adapter`: parse or mapping failure;
- `dataset`: leakage, missing rows, insufficient negatives;
- `model`: training/inference/artifact failure;
- `guard`: action rejected by declared policy;
- `verification`: actual state differs from expected state.

## No-Fallback Examples

Reject these patterns in code review:

- `unwrap_or_default()` on missing vectors;
- returning an empty vector when a CF read fails;
- treating deserialization failure as "record not found";
- substituting a previous model artifact when the requested artifact is missing;
- skipping invalid dataset rows without incrementing a persisted reason count;
- using in-memory counts as proof instead of RocksDB readback;
- unit tests that validate only return values when a source-of-truth write is expected.

## Audit Log

DynamicJEPA audit rows should include:

- audit id;
- timestamp;
- operation;
- actor/tool;
- input ids;
- output ids;
- CF names touched;
- content hashes;
- error if failed;
- verification run id when applicable.

Audit rows are append-only. They are not a substitute for primary records, but they help reconstruct a workflow.

## Verification Run Records

`VerificationRunRecord` stores:

- verification run id;
- test name;
- db path hash or identifier;
- source fixture hashes;
- before counts;
- commands executed;
- after counts;
- decoded record excerpts;
- artifact hash checks;
- expected results;
- actual results;
- status;
- failure details.

Manual tests should persist verification runs so evidence survives terminal scrollback.

## Operator Debugging Flow

When something fails:

1. Read the error message and operation id.
2. Read the relevant run record.
3. Inspect the source-of-truth CF count.
4. Read linked input and output records by id.
5. Check artifact file hashes if model code was involved.
6. Compare expected count delta to actual count delta.
7. Fix the lowest-level broken invariant.
8. Re-run the full happy path and the failing edge case.

## Logging Tests

Tests should verify errors include context:

- operation name;
- CF name when storage related;
- record id when known;
- field path for schema failures;
- expected and actual version for codec failures;
- remediation hint for operator action.

Do not test exact entire strings unless necessary. Test required fields and stable substrings.
