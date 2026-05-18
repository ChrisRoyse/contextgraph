# FSV-PROTOCOL - Mandatory Verification Contract

> **Authority.** This is the stable task-level verification path referenced by
> the futurebuild docs and `CLAUDE.md`. The universal operating contract lives in
> `docs2/AICodingAgentSuperPrompt.md`; this file applies that contract to
> ContextGraph futurebuild tasks.

---

## 1. Source Of Truth First

A return value is a claim. The Source of Truth is the verdict.

Before declaring any implementation complete, answer all four questions:

1. **What state changed?** File, RocksDB CF row, metric, queue item, report, or
   external record.
2. **Where does that state live?** Exact path, CF name and key, metric name, or
   external identifier.
3. **How is it read back independently?** Separate DB open, file read,
   checksum, source command, or equivalent.
4. **What exact value is expected?** Literal value, schema, count delta, hash,
   or bounded range.

If any answer is unknown, the feature is not ready to call done.

## 2. Trigger -> Process -> Outcome

Every FSV-able feature has this shape:

```text
[Trigger] -> [Process] -> [Outcome]
 observable   measurable   verifiable at SoT
```

Document these before implementation:

- **Trigger:** MCP tool call, CLI command, scheduler tick, hook event, file
  event, or operator action.
- **Process:** Code path and functions that execute.
- **Outcome:** Physical end state to read back, including exact path, CF, key,
  and expected value.

## 3. Hard Rules

1. No mocks for completion gates.
2. No fallbacks that hide required-output failure.
3. No verifier weakening, ignored tests, or silent bypasses.
4. Tests must fail when the feature is broken.
5. Missing CFs, files, models, env vars, or malformed inputs fail closed with
   structured `SCREAMING_SNAKE_CASE` errors.
6. Assume the task is wrong until live files, symbols, and CF names prove it is
   current.
7. Prefer the smallest implementation that satisfies the issue and matches
   local patterns.

## 4. Synthetic Data Properties

Synthetic FSV input is allowed when it is deterministic, distinguishable,
representative, boundary-rich, privacy-safe, and cleanup-tagged. Mocked
subsystems are not acceptable completion evidence.

## 5. Boundary Cases

Cover at least three applicable boundary classes per code path:

1. Empty or missing input.
2. Single item.
3. Maximum allowed.
4. Maximum plus one.
5. Minimum allowed.
6. Minimum minus one.
7. Wrong type or malformed input.
8. Unicode, control-byte, or very long string edges.
9. Duplicate or replayed event.
10. Out-of-order event.
11. Concurrent writers or sessions.
12. Authorization or tenant-scope variants.
13. Time edges: skew, negative timestamp, DST, leap second, month boundary.
14. Resource exhaustion: full disk, OOM, pool exhaustion, rate limit.

Each boundary case needs before state, trigger/action, after state, expected
value, actual value, and pass/fail.

## 6. Required FSV Evidence

For implementation tasks, produce:

1. **Build evidence:** `cargo build --release -p <touched-crate>`.
2. **Test evidence:** `cargo test -p <touched-crate>` or a scoped test with a
   stated reason.
3. **Clippy evidence:** `cargo clippy -p <touched-crate> --no-deps --tests
   --examples -- -D warnings`.
4. **SoT readback artifact:** production FSV must be written on aiwonder under
   `/zfs/archive/contextgraph/fsv/<task>-fsv/<artifact>.json` with a pass field,
   exact SoT path, happy path, boundary cases, and physical artifact hashes or
   counts. `/mnt/d/contextgraph/fsv/...` is local-only evidence unless an issue
   explicitly scopes a local smoke and names that limitation in closeout.
5. **Independent RocksDB verification** for CF writes: flush/drop/reopen through
   the production opener or an equivalent separate handle, then decode the same
   row back.
6. **GitHub issue closeout:** CLAIM before code work, then RESOLVED / PAUSE /
   BLOCKED comment with command evidence, FSV path, hashes, and remaining risk.

Documentation-only tasks may replace build/test/clippy with direct file
existence, checksum, link/reference, and GitHub-state readbacks.

Minimum SoT evidence shape:

```json
{
  "task": "TASK-EXAMPLE-NNN",
  "sourceOfTruth": "<CF/key or absolute file path>",
  "trigger": "<exact command or action>",
  "expected": "<expected value>",
  "actual": "<actual value read independently>",
  "cases": [],
  "allPassed": true
}
```

## 7. Audit-The-Task Checklist

Run this before implementation:

```bash
test -f <path>
rg "pub (struct|enum|fn) <symbol>" crates/ --type rust
rg "CF_MEJEPA_<NAME>" crates/
test -f docs/futurebuild/specs/FSV-PROTOCOL.md
git log --oneline -5 -- <path>
```

If the audit contradicts the task, update the issue or escalate before writing
code.

## 8. Research And Competing Approaches

When a task needs an unfamiliar algorithm, dependency, protocol, or standard,
read primary sources first. Capture the chosen approach in the issue comment,
including why alternatives were rejected and what evidence would justify
revisiting the decision.

## 9. Occam Evaluation

For non-trivial designs, record in the issue:

- Approach chosen.
- Alternatives considered.
- Why the chosen approach has fewer moving parts or stronger SoT evidence.
- What future evidence would change the decision.

## 10. Failure Modes That Mean Not Done

Green tests are not sufficient if any of these are true:

- The test checks a constant instead of exercising the implementation.
- The tested branch is inactive.
- A CF row is written but never flushed or reopened.
- The test uses a different opener or serializer than production.
- Missing values are hidden with defaults.
- Async producer tasks are spawned but not joined before readback.
- Evidence proves existence only, not field-level correctness.

## 11. Issue Closeout Contract

Every worked issue ends with exactly one of:

- **RESOLVED** when code/docs and FSV evidence are complete.
- **PAUSE** when work is partially complete and another session needs a resume
  point.
- **BLOCKED** when a named dependency or operator decision prevents progress.

The comment must include files changed, commands run, FSV artifact path and hash,
SoT readback summary, failures encountered, residual risk, and follow-up issues.
No `./memory/` files are written.

## 12. Evidence That Cannot Be Skipped

The evidence must include:

- A true pass verdict in the artifact (`allPassed` or local equivalent).
- Counts before/after for any CF the task writes.
- Field-level readback, not merely "row exists".
- Physical file/database artifact proof.
- Boundary-case detail strings naming the invariant tested.
- A clear SoT path that a fresh agent can read without trusting the claim.

Anything less is not FSV; it is a claim waiting to be verified.

## Cross-References

- `docs2/AICodingAgentSuperPrompt.md` - universal doctrine and issue protocol.
- `CLAUDE.md` - repository-level operating rules.
- `docs/futurebuild/README.md` - current futurebuild doc map.
- `docs/futurebuild/08_roadmap_and_open_work.md` - locally synced roadmap and current open-work summary.
