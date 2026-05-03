# Training And Eval Corpus

BlackBoxFix turns verified agent operations into high-quality training and
eval data. The key is that each row includes the **expected delta** and the
**actual readback**, not just a transcript.

This document is the contract for the **shipped Phase 5** exporter
(`blackbox export episodes`). The existing DynamicJEPA exporter was the
implementation pattern copied for fail-closed JSONL export, manifest writing,
and physical readback.

## Reference Implementation Pattern

DynamicJEPA already ships an export pipeline with the same shape:

- Implementation: `crates/context-graph-cli/src/commands/export_dynamicjepa.rs`
- FSV test: `crates/context-graph-cli/tests/dynamicjepa_bundle_export_fsv.rs`
- CLI surface: `context-graph export dynamicjepa-episodes` (see
  `crates/context-graph-cli/src/main.rs:158-161,204-207`).

Phase 5 mirrors that pipeline: read the physical SQLite source of truth,
re-parse every line of the JSONL after writing, hash the file, and write a
manifest that points at the exact ledger sequence range exported.

## Episode Row Contract

```json
{
  "schema_version": 11,
  "episode_schema_version": 1,
  "record_kind": "verified_agent_episode",
  "dataset_split": "passed",
  "episode_id": "run_123:step_4:tool_2",
  "source": {
    "tenant_id": "tenant_...",
    "run_id": "run_123",
    "step_id": "step_4",
    "tool_call_id": "tool_2",
    "evidence_packet_id": "ev_...",
    "ledger_sequence_range": [42, 47],
    "ledger_tip_sha256_at_export": "sha256:..."
  },
  "state": {
    "task": "upgrade vulnerable dependency",
    "policy_context": {},
    "available_tools": [],
    "source_of_truth": []
  },
  "action": {
    "tool_name": "github_create_pull_request",
    "tool_input_redacted": {},
    "model_decision_redacted": {}
  },
  "expected_persisted_delta": {},
  "actual_readback": {},
  "verifier_status": {
    "status": "passed",
    "score": {}
  },
  "error_code": null,
  "remediation": null
}
```

Hashes (`source.ledger_tip_sha256_at_export` and
`manifest.output.episodes_sha256`) are required so a verifier can prove the
exporter saw the same physical state and wrote the bytes described by the
manifest. If the ledger has been extended after export, the manifest's
`ledger_tip_sha256_at_export` will not match the latest tip — that is
intended; readers can still recompute the chain across the exported range and
confirm it matches.

## Data Splits

The exporter must support these splits and never silently drop a category:

- `passed`: correct actions.
- `failed`: outcome mismatch examples.
- `blocked`: governance and precondition failures.
- `incident`: high-risk failure packets.
- `edge_case`: manual FSV edge-case rows.

**Failures are the most valuable training data.** Do not discard them.

## Training Uses

### Process Supervision

Train models to prefer actions that lead to verified state, not just plausible
text. The model sees: task state, action, expected delta, actual readback,
verifier result.

### Tool-Use Fine-Tuning

Teach exact tool schema use, precondition checking, and remediation.

### Eval Generation

Turn each failure class into an eval:

- prompt injection attempts;
- unauthorized tool invocation;
- missing readback permission;
- wrong source-of-truth state;
- incomplete remediation;
- policy-denied action.

### Recovery Training

Use failed and blocked episodes to train the next action:

```json
{
  "state": { "verifier_status": "failed", "error_code": "CI_REQUIRED_CHECK_FAILED" },
  "expected_next_action": {
    "tool_name": "ci_get_test_artifact",
    "reason": "inspect failing test before editing again"
  }
}
```

## Export Modes

| File | Purpose |
|---|---|
| `episodes.jsonl` | one episode per line, UTF-8, no trailing newline weirdness |
| `manifest.json` | counts per split, sha256 of `episodes.jsonl`, source ledger ids, schema versions, exporter version, ledger tip hash |
| `evals/` | generated regression eval fixtures for non-passed rows |

## Export Acceptance Rules

The exporter MUST:

1. Read from the physical ledger (`SELECT * FROM ledger_records ORDER BY
   sequence`) — not in-memory return values.
2. Reject broken hash chains (`BBX_HASH_CHAIN_BROKEN`) before writing any
   row. This is the same `verify_ledger_integrity` sweep used by
   `inspect ledger`.
3. Reject missing verifier results for mutating actions (Phase 2+).
4. Re-parse every JSONL row after writing (read back, deserialize, assert).
5. Compute and store SHA-256 of `episodes.jsonl` in `manifest.json`.
6. Record the exact `ledger_sequence` range covered.
7. Refuse to overwrite an existing output directory unless `--force`.
8. Reject any mutating tool call that does not have an evidence packet.
9. Write through a temporary directory and promote only after JSONL, manifest,
   and eval fixture readback all succeed.

## High-Value Dataset Types

- agent action → verifier result;
- failed action → remediation;
- task → required source-of-truth checks;
- tool schema → valid invocation;
- policy context → allowed or denied tool use;
- before/after state → expected delta;
- incident packet → root cause label.

## Long-Term Moat

A corpus of verified operational episodes is hard to copy because it comes
from real tool use, real source-of-truth readback, and real failures. This
becomes the data layer for agents that understand consequences.

## FSV Of The Phase 5 Exporter (shipped)

Synthetic happy path:

```bash
ledger=/tmp/bbx-export.sqlite
out=/tmp/bbx-export-out
context-graph blackbox init --ledger "$ledger"
# ... run + tool-calls produced via Phase 1 commands ...
context-graph blackbox export episodes --ledger "$ledger" --out "$out"

# Source of truth checks (must all pass)
test -f "$out/episodes.jsonl"
test -f "$out/manifest.json"
jq -e '.counts.episodes_total > 0 and .output.episodes_sha256 != ""' "$out/manifest.json"
sha256sum "$out/episodes.jsonl" | awk '{print "sha256:" $1}' \
  | diff - <(jq -r .output.episodes_sha256 "$out/manifest.json")
# Re-parse each line; every line must be valid JSON with required fields:
while IFS= read -r line; do
  echo "$line" | jq -e '.episode_id and .verifier_status.status' >/dev/null
done < "$out/episodes.jsonl"
```

Edge cases (must produce typed errors, no rows written):

1. `--out` already exists without `--force` → exit 1, error `BBX_OUTPUT_EXISTS`.
2. Ledger has a hash-chain break → exit 2, error `BBX_HASH_CHAIN_BROKEN`,
   no manifest written.
3. Mutating tool call missing a verifier result (Phase 2+) → exit 1, error
   `BBX_VERIFIER_MISSING_FOR_MUTATING_ACTION`.

Each edge case has a corresponding test in
`crates/context-graph-cli/tests/blackbox_phase5_export_fsv.rs`.

Manual FSV completed on 2026-05-02:

```text
Log: /tmp/bbx-phase5-manual-fsv.exRXYj/fsv.log
Ledger source of truth: /tmp/bbx-phase5-manual-fsv.exRXYj/blackbox.sqlite#ledger_records
Export source of truth: /tmp/bbx-phase5-manual-fsv.exRXYj/episodes-export/
Final ledger counts: agent_run=1, tool_call=1, expected_delta=2,
readback_snapshot=2, verifier_result=2, evidence_packet=2, incident_packet=1
Export rows: 3 total; splits passed=1, failed=1, incident=1
Export SHA: sha256:c0c4d0a76db8ed18ba2c708208f6556de5040a9854ca51a1cf44f069c53afcda
Eval fixtures: 2
```
