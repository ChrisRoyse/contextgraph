# Tests, Acceptance, And Full-State Verification

Testing must prove the system works by inspecting durable state after
execution. Unit tests are useful, but they are not enough for this product.
The mandatory operator-side protocol lives in
`19_full_state_verification_protocol.md`; this file lists the testing
**levels** and the per-phase acceptance criteria.

## Source Of Truth For BlackBoxFix Itself

| Source | What it proves |
|---|---|
| SQLite ledger DB at `--ledger PATH` | every row written, every hash chain |
| `blackbox_metadata` rows | record count + tip hash agree with physical rows |
| Evidence packet rows + exported report files (Phase 4) | packet references match physical rows and export files read back byte-for-byte |
| Hash-chain verification output | per-record + per-run chain unbroken |
| External systems under connector tests | readback matches expected delta |
| Exported JSONL + manifest (Phase 5) | every line re-parses; sha256 matches manifest |
| MCP tool registry + policy rows (Phase 6) | advertised tools match shipped schemas; governed decisions persist in `ledger_records` |
| Dashboard bundle (Phase 7) | `index.html`, `manifest.json`, and `api/*.json` read back from disk; manifest sha256 entries match actual bytes |

## Test Levels

### Unit Tests

- schema validation;
- redaction (field-name + literal pattern + post-scan);
- canonical-JSON sorting;
- hash computation;
- comparison functions;
- score calculation;
- error-code mapping.

### Integration Tests

- connector readback against real local systems (no mocks);
- ledger write and readback on a real SQLite file;
- evidence artifact hash verification;
- exporter parse and manifest verification.

### Full-State Verification (FSV)

For each mutating workflow, follow the protocol in
`19_full_state_verification_protocol.md`:

1. Print the before-state from the source of truth.
2. Execute the action.
3. Print the after-state from the source of truth through a separate read.
4. Verify the expected persisted delta matches.
5. Print evidence packet ids and ledger row sequences.
6. Run at least three edge cases.

Phase 1 has automated FSV (`tests/blackbox_phase1_fsv.rs`). Phase 2 has
automated FSV (`tests/blackbox_phase2_fsv.rs`) against real SQLite ledgers,
real Git repositories, real files, and real commands. Phase 6 adds
`tests/blackbox_phase6_mcp_policy_fsv.rs`, which executes the real MCP stdio
gateway against a real Git patch and then reads SQLite/Git state directly.
Phase 7 adds `tests/blackbox_phase7_dashboard_fsv.rs`, which builds a real
dashboard bundle and reads SQLite/dashboard files directly.

## Acceptance Criteria

The MVP is accepted only when, **after running on a fresh checkout against
real systems**:

- every mutating action has a verifier or is blocked in governed mode;
- every verifier has happy path and three edge cases;
- every failure has error code, source of truth, field path, and remediation;
- every evidence packet can be read back from storage;
- every exported JSONL row links to evidence;
- hash-chain tampering is detected;
- redaction failures stop persistence;
- connector permission failures are not reported as success.

## Manual FSV Template

```text
FSV: <verifier_id>
Source of truth: <system + locator>

Before:
  <field>: <value>

Trigger:
  <command or tool call>

After readback (via independent read path):
  <field>: <value>

Expected delta:
  <field>: <value>

Verifier:
  status: passed | failed | blocked
  error_code: <or null>
  evidence_packet_id: <ev_...>

Physical proof:
  <sqlite/git/api command output>
```

## Required Edge Cases (per verifier)

Mandatory three:

1. **Missing target state** (e.g. PR not found, branch missing, file absent).
2. **Wrong target state** (e.g. PR closed, file content mismatch).
3. **Permission or schema failure** (connector rejected; expected
   `BBX_*` or `*_PERMISSION_DENIED`).

Strongly recommended additional edge cases:

- stale readback;
- partial write;
- duplicate action;
- rate limit;
- redaction violation;
- artifact hash mismatch;
- tampered ledger record (Phase 1 already covers this).

## Real Data Rule

Tests use real local repositories, real temporary databases, real files, real
command output, and real connector sandboxes. **No test passes by asserting a
fabricated success response while the source of truth is broken.**

Concretely, that means:

- Phase 1 FSV opens the SQLite file with the standalone `rusqlite` library
  and queries `ledger_records` directly, then asserts row counts and
  payload contents — see `phase1_happy_path_persists_and_inspects_physical_rows`.
- Phase 2 FSV shells to the real `git` binary against a real test repo and
  uses direct `git rev-parse`/`git cat-file`/`git status`/`git diff` plus
  direct SQLite reads before asserting verifier output.
- Phase 3 FSV points at live GitHub/GitHub Actions state and writes only local
  SQLite rows.
- Phase 4 FSV reads `evidence_packet.record_refs[]` back from physical
  SQLite rows, verifies replay is read-only by comparing before/after counts,
  writes a real JSON export file, reads the file back from disk, byte-compares
  it, and parses it before success.
- Phase 5 FSV exports real `episodes.jsonl`, `manifest.json`, and eval
  fixtures, then reads every file back from disk, re-parses every JSONL line,
  checks the manifest SHA against actual bytes, and verifies every episode's
  evidence packet id exists in physical SQLite rows.
- Phase 6 FSV drives `blackbox mcp gateway` over real JSON-RPC stdio, applies
  a real Git patch, then independently reads `ledger_records` and `git diff`
  to prove `policy_decision`, `agent_step`, `model_decision`, `tool_call`,
  verifier, and `evidence_packet` rows exist only when expected.
- Phase 7 FSV builds `index.html`, `manifest.json`, and `api/*.json` from a
  real ledger with one passed run and one failed run, reads each file back
  from disk, verifies manifest hashes against actual bytes, and checks every
  dashboard evidence id exists in physical SQLite rows.

For hosted systems unavailable in CI, run those tests as gated integration
tests with a `BBX_LIVE_GITHUB_TOKEN` env var; keep all local source-of-truth
tests **mandatory** (gating only the live ones is a recipe for green-on-broken).

## Synthetic Data Strategy

For each phase, synthesize input where you control the ground truth, then
read the source of truth and compare. Phase 1 inputs/outputs are enumerated
in `20_phase1_state_and_invariants.md` §"Happy Path", §"Edge Case 1/2/3".
Phase 2 inputs/outputs are enumerated in
`21_phase2_state_and_invariants.md`. Phase 4/5/6/7 inputs/outputs are
enumerated in `23_phase4_state_and_invariants.md`,
`24_phase5_state_and_invariants.md`,
`25_phase6_state_and_invariants.md`, and
`26_phase7_state_and_invariants.md`.

A synthetic test must include:

1. The exact command(s) issued.
2. The exact JSON file(s) supplied as input.
3. The expected stdout JSON shape (success) **or** stderr JSON shape
   (failure with `error_code`).
4. The expected `sqlite3 …` query and its expected output.
5. The expected exit code (`0`, `1`, or `2`).

If any of these is missing, the test is incomplete.

## Regression Evals

Every production incident and every new verifier edge case produces:

- a ledger fixture (a deterministic SQLite file under `tests/fixtures/`);
- an episode row;
- an eval case;
- expected verifier status;
- expected remediation.

A regression that ships without all five is not a regression test; it is a
note.

## Tying It Together

The shortest valid sentence describing acceptance: **"When I run
`cargo test --package context-graph-cli --test blackbox_phaseN_fsv` on a
fresh checkout against real systems, all tests pass and a manual run of
the documented commands produces byte-identical SQLite/JSON output."**
