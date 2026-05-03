# Full-State Verification Protocol (mandatory)

**Read this file before writing or modifying any BlackBoxFix code.** This is
the operator-side contract: every change must complete the protocol below
before being declared "done". A passing return value is not enough — the
durable source of truth must be inspected directly.

This protocol applies to **every phase** of the build plan in
`13_mvp_implementation_phases.md`.

## Why This Protocol Exists

In computing, a trigger event causes a process X, which produces an outcome
Y. Because the trigger is observable, and Y must be inspectable for the
product to have any value, every step in the pipeline can be verified by
reading the source of truth directly. **Anywhere that read is skipped is a
place where a bug is allowed to live.**

The Occam's-razor framing: the simplest correct way to know the system works
is to read the system after using it. Anything more elaborate (mocks,
return-value asserts, indirect telemetry) can lie. The physical row, file,
or API response cannot.

## Step-By-Step Protocol

### 1. Identify The Source Of Truth

Every change has at least one source of truth. State it explicitly before
writing a line of code:

| Phase | Source of truth |
|---|---|
| 1 | SQLite file at `--ledger PATH`, table `ledger_records`, plus `blackbox_metadata` rows for `ledger_record_count` / `ledger_tip_sha256` |
| 2 | Phase 1 + on-disk Git repository state + filesystem files + test command exit codes |
| 3 | Phase 2 + GitHub API (PRs, checks, issues) + GitHub Actions API |
| 4 | Phase 3 + evidence/incident packet rows + exported evidence report file |
| 5 | Phase 4 + exported `episodes.jsonl`, `manifest.json`, and `evals/` on disk |
| 6 | Phase 5 + MCP tool registry contents + `policy_decision`, `agent_step`, `model_decision` rows + physical Git state for gateway mutations |
| 7 | Phase 6 + dashboard bundle files (`index.html`, `manifest.json`, `api/*.json`) and dashboard API responses |
| 8 | Phase 7 + `enterprise_tenant_config`, `enterprise_directory_snapshot`, `enterprise_access_decision` rows + audit export files (`audit.jsonl`, `siem-rfc5424.log`, `controls.json`, `manifest.json`) |
| 9 | Phase 8 + live GitLab REST project/pipeline/job API readbacks |
| 10 | Phase 9 + physical OSV report, Cargo lockfile, and CycloneDX SBOM files |
| 11 | Phase 10 + live GitHub issue body/comment API readbacks |
| 12 | Phase 11 + `scorecard` rows read directly from SQLite |
| 13 | Phase 12 + `retention_decision` and `evidence_deletion` rows read directly from SQLite |
| 14 | Phase 13 + `release_gate` rows read directly from SQLite |
| 15 | Phase 14 + `pilot_package` rows and package/dashboard JSON/JSONL files |
| 16 | Phase 15 + `risk_assessment` rows and real decision-register JSON files |
| 17 | Phase 16 + real HTTP requests to `blackbox server serve` and `http_request_audit` rows read directly from SQLite |
| 18 | Phase 17 + `first_wedge_demo` rows, real Git/scanner/GitHub/GHA verifier evidence, and demo package JSON/JSONL files with manifest hashes |

If you cannot name the source of truth in one sentence, stop and design the
data model first.

### 2. Capture Pre-State Independently

Use a read path that does **not** go through the code being tested. Examples:

- `sqlite3 <ledger> 'SELECT COUNT(*) FROM ledger_records;'` instead of calling
  the same function the code uses.
- `git log -1 --format=%H` instead of asking your code "what is the head
  commit".
- `curl https://api.github.com/repos/.../pulls/N` instead of calling the
  GitHub connector.

Print the pre-state. Hash it if practical (`sha256sum`).

### 3. Execute The Trigger Action

Run the command, function, or HTTP call that should change the system. Record
its exit code and full stdout/stderr.

### 4. Read The Source Of Truth Through An Independent Path

Repeat step 2 after the action. Never rely on the action's own return value
for the after-state.

### 5. Compute The Delta

Print:

- pre-state;
- expected delta (what the code claims it changes);
- actual delta (post-state minus pre-state);
- the field-by-field comparison.

### 6. Verify Or Fail

If actual delta == expected delta on every field, the change is verified.
If not, emit a typed failure with `error_code`, `field_path`, `source_of_truth`,
and `remediation`.

### 7. Boundary And Edge Case Audit

Manually simulate at least three edge cases. For each:

- print the system state before;
- run the action with the edge-case input;
- print the system state after;
- assert the typed error code matches the spec.

The minimum three edge cases for any mutating action:

1. **Missing target state** — the resource the action depends on does not
   exist (empty input, missing parent row, missing file).
2. **Wrong target state** — the resource exists but is in a state the action
   should refuse (closed PR, mismatched branch, schema drift).
3. **Permission or schema failure** — the connector / store rejects the
   action (no auth, malformed input, ledger uninitialized).

Add more edge cases when the data invariants demand it (see Phase 1's
hash-chain tampering test for an integrity-class edge case).

### 8. Provide Evidence Of Success

The evidence log must include:

- the exact CLI command(s) executed;
- the exact input file(s) consumed;
- the exact stdout JSON (success path) or stderr JSON (failure path);
- the exit code;
- the exact `sqlite3 …` / `git …` / `curl …` queries used to read the source
  of truth;
- the output of those queries showing the persisted state.

If any of these is missing, the evidence is incomplete.

### 9. Re-Run The Automated FSV Suite

Run the appropriate `tests/blackbox_phaseN_*_fsv.rs` file on a fresh checkout:

```bash
cargo test --package context-graph-cli --test blackbox_phaseN_fsv -- --nocapture
```

Every assertion in those tests reads the physical source of truth (open the
SQLite file, run a `git` command, hit the API). A passing test that does
**not** physically inspect the system is a defect — replace it.

## Anti-Patterns To Refuse

- "The function returned `Ok(())`, so it works."
- "I asserted on the in-memory struct that we just constructed."
- "I added a feature flag so old code still passes."
- "We catch the error and fall back to writing nothing."
- "We mock the database so the test runs faster."
- "I'll write the FSV test after the feature ships."

Each of those statements is grounds for rejecting the change.

## Synthetic Test Inputs Are Mandatory

For every mutating action you build, write the synthetic input where you
already know the expected output (because you constructed it). Then read the
source of truth and prove the output is what you predicted.

Phase 1 example (already runnable):

| # | Action | Input you control | Source-of-truth read | Expected output |
|---|---|---|---|---|
| 1 | `blackbox init` | new path | `sqlite3 PATH '.tables'` | `blackbox_metadata`, `ledger_records` |
| 2 | `record run` with `--task=t --agent=a` | a / t | `SELECT record_kind, payload_json FROM ledger_records;` | one row, `record_kind='agent_run'`, payload contains `requested_outcome='t'` |
| 3 | `record tool-call` with `--input=file containing 'api_key': 'abc'` | the JSON file | `SELECT redaction_json, payload_json FROM ledger_records WHERE record_kind='tool_call';` | `redaction_json.redacted_fields=['$.api_key']`; payload shows `[REDACTED:sha256:...]` |
| 4 | `inspect ledger` after privileged tamper | drop trigger; UPDATE | exit code | `2`, `error_code=BBX_HASH_CHAIN_BROKEN` |

The complete set lives in `20_phase1_state_and_invariants.md`.

## Definition Of Done For A Phase

A phase is done when **all** of the following hold simultaneously:

- [ ] Every exit criterion from `13_mvp_implementation_phases.md` for the
      phase is checked off.
- [ ] A `tests/blackbox_phaseN_*_fsv.rs` file exists and passes.
- [ ] The manual FSV protocol above has been run and the evidence log is
      attached to the PR.
- [ ] Every error path emits a typed `BBX_*` (or phase-specific) error code
      with `source_of_truth`, `connector`, `field_path`, and `remediation`.
- [ ] No backwards-compatibility shims, fallbacks, or migration adapters
      were added.
- [ ] On a schema bump, the SQLite ledger is wiped (delete the file in dev,
      document the wipe procedure in the PR for ops).
- [ ] The relevant docs in this directory have been updated; the
      `00_master_index.md` "Repository State" block reflects what shipped.

If any box is unchecked, the phase is not done.
