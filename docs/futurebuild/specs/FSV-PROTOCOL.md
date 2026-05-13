# FSV-PROTOCOL — Mandatory Verification Contract

> **Authority.** This is the task-level verification contract referenced by the
> futurebuild docs, `CLAUDE.md`, and `docs/MEJEPA_SUPER_PROMPT.md`. The full
> operational synthesis lives in `docs/MEJEPA_SUPER_PROMPT.md §14`; this file is
> the stable source path for agents and issue definitions that say a task
> "inherits FSV-PROTOCOL.md".
>
> A task is not complete because code compiles, tests pass, or a function
> returns `Ok(())`. A task is complete only when the changed state is read back
> from its Source of Truth (SoT) and the evidence covers the happy path plus
> relevant boundary cases.

---

## 1. First Principle: Return Values Lie; SoT Does Not

A process exiting `0`, an HTTP `200 OK`, a log line saying `persisted`, and a
function returning success are all claims. They are useful evidence that an
attempt occurred, but they are not proof that the intended state changed.

Before declaring work complete, answer all four questions:

1. **What state changed?** A file, RocksDB CF row, metric, queue item, report, or
   external record.
2. **Where does that state live?** Exact file path, CF name and key, metric name,
   or external identifier.
3. **How is it read back independently?** Separate DB open, file read, checksum,
   `Storage::get_cf`, command output, API read, or equivalent.
4. **What exact value is expected?** Literal value, schema, count delta, hash, or
   bounded range.

If any answer is unknown, the design is not ready to call done.

## 2. Trigger -> Process -> Outcome

Every FSV-able feature has this shape:

```text
[Trigger] -> [Process] -> [Outcome]
 observable   measurable   verifiable at SoT
```

Document these before implementing:

- **Trigger:** MCP tool call, CLI command, cron tick, shift-log append, hook
  event, file event, or operator action.
- **Process:** Code path and functions that execute.
- **Outcome:** Physical end-state to read back, including exact path/CF/key.

If the trigger is not observable or the outcome has no independent readback,
fix the design before writing implementation code.

## 3. Hard Rules

1. **No mocks for completion gates.** Use real RocksDB temp roots, real files,
   real production openers, real serializers, real CLIs, and real tool paths
   wherever the production code would.
2. **No backwards compatibility shims.** This is pre-production. Fail fast with
   structured errors instead of adding migration probes or silent fallbacks.
3. **No workarounds.** Fix the underlying issue or record a blocker. Do not
   weaken verifiers or add `#[ignore]` to ship.
4. **Tests must fail when the feature is broken.** Mutation-test mentally. If
   removing the implementation leaves the test green, the test is wrong.
5. **Fail closed on unknowns.** Missing CF, file, model, env var, or malformed
   input must produce a structured `SCREAMING_SNAKE_CASE` error.
6. **Assume the task is wrong.** Audit file paths, symbols, CF names, and docs
   references against the live tree before editing.
7. **Occam's razor.** Prefer the smallest implementation that satisfies the
   definition of done and matches local patterns.

## 4. Synthetic Data Properties

Synthetic FSV inputs must be deterministic, distinguishable, representative,
boundary-rich, privacy-safe, and cleanup-tagged. Synthetic data is acceptable as
input; mocked subsystems are not acceptable as completion evidence.

## 5. Mandatory Boundary Case Set

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

Each boundary case needs an evidence line with before state, trigger/action,
after state, expected value, actual value, and pass/fail.

## 6. Required FSV Evidence Artifacts

For implementation tasks, produce:

1. **Build evidence:** `cargo build --release -p <touched-crate>`.
2. **Test evidence:** `cargo test -p <touched-crate>` with zero failed tests.
3. **Clippy evidence:** `cargo clippy -p <touched-crate> --no-deps --tests
   --examples -- -D warnings`.
4. **SoT readback JSON:** `/mnt/d/contextgraph/fsv/<task>-fsv/<artifact>.json`
   with `all_passed: true`, happy path, boundary cases, exact commands, and
   physical artifact paths.
5. **Physical artifact verification:** file size/hash, RocksDB SST listing, or
   metric/readback proof.
6. **Independent reopen verification for RocksDB:** close, reopen in a separate
   handle/process, then read the same row back.
7. **Session report:** `./memory/reports/session-YYYY-MM-DD--<slug>.md` with
   modified files, evidence paths, commands, failures, and resume point.

Documentation-only tasks may replace build/test/clippy with direct file
existence, checksum, link/reference, and GitHub-state readbacks, but they still
need explicit evidence that the referenced state now exists.

Minimum SoT evidence shape:

```json
{
  "task_id": "TASK-EXAMPLE-NNN",
  "source_of_truth": "<CF/key or absolute file path>",
  "trigger": "<exact command or action>",
  "expected": "<expected value>",
  "actual": "<actual value read independently>",
  "boundary_cases": [],
  "all_passed": true
}
```

## 7. Audit-The-Task Checklist

Run this before implementation:

```bash
# Does each referenced file exist?
test -f <path>

# Are referenced symbols present?
rg "pub (struct|enum|fn) <symbol>" crates/ --type rust

# Are referenced CFs still registered/opened?
rg "CF_MEJEPA_<NAME>" crates/

# Do docs/spec refs still exist?
test -f docs/futurebuild/specs/FSV-PROTOCOL.md

# What changed recently in this area?
git log --oneline -5 -- <path>
```

If the audit contradicts the task, update the issue/task or escalate before
writing code.

## 8. Research And Competing Approaches

When a task needs an unfamiliar algorithm, dependency, protocol, or standard,
read primary sources first. Capture 2-3 approaches, why the chosen option wins,
and what evidence would justify revisiting it. Never vendor or path-depend on
upstream reference implementations; use them as clean-room references only.

## 9. Occam Evaluation Template

Use this in the session report for non-trivial designs:

```markdown
- Approach chosen: <one-line description>
- Considered alternatives:
  1. <alternative> — rejected because <specific reason>
  2. <alternative> — rejected because <specific reason>
- Why this wins: <fewest moving parts / matches existing pattern / stronger SoT>
- What changes the decision: <future evidence>
```

## 10. Failure Modes That Mean "Not Done"

Green tests are not sufficient if any of these are true:

- The test checks a constant instead of exercising the implementation.
- The tested feature flag or branch is not active.
- A CF row is written but never flushed or reopened.
- The test uses a different DB opener or serializer than production.
- JSON deserialization silently swallows renamed fields.
- Missing values are hidden with `unwrap_or_default`.
- Async producer tasks are spawned but never joined before readback.
- Evidence proves only existence, not field-by-field equality.

## 11. Session Report Contract

End every task with a memory report that states:

- Outcome: complete, partial, or blocked.
- Files modified.
- FSV artifacts and absolute paths.
- Build/test/clippy or docs-readback evidence.
- Decisions and discoveries.
- Failures and lessons.
- Open blockers.
- Resume point for the next session.

## 12. What Evidence Cannot Skip

The evidence must include:

- `all_passed: true` for completed FSV artifacts.
- Counts before/after for any CF the task writes.
- Field-by-field readback equality, not merely "row exists".
- Physical file/database artifact proof.
- Boundary-case detail strings naming the invariant tested.
- A clear SoT path that a fresh agent can read without trusting your claim.

Anything less is not FSV; it is a claim waiting to be verified.

## Cross-References

- `docs/MEJEPA_SUPER_PROMPT.md §14` — expanded operational FSV contract.
- `CLAUDE.md` — repository-level operating rules.
- `docs/futurebuild/00_INDEX.md` — futurebuild doc map and spec hierarchy.
- `docs/futurebuild/13_TASK_INDEX.md` — task DAG and inheritance statement.
