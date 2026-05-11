# FSV-PROTOCOL — Mandatory Verification Contract for Every Task

> **Authority.** This document is the inherited verification contract for every
> `TASK-*.md` file under `docs/futurebuild/tasks/`. When a task says "FSV
> evidence path: …" or "Test boundary cases: …", the *content* of those
> sections is satisfied by following this protocol. **Implementing a task
> without producing the artifacts in §6 means the task is not done — no
> matter what `cargo test` says.**
>
> Audience: any AI coding agent (or human) with a fresh context window
> picking up a `TASK-*` file. Read this once at the start of every task; the
> task file is the *what*, this document is the *how-do-I-prove-it-worked*.

---

## 1. First principle: return values lie; the Source of Truth does not

A function returning `Ok(())`, a process exiting `0`, a log line saying
`"persisted"`, and an HTTP `200 OK` are all *claims*. They are evidence the
attempt occurred. They are **not** evidence the work happened. The only
evidence that counts is a separate read against the Source of Truth (SoT)
showing the bytes you expected, where you expected them.

You MUST be able to answer these four questions before declaring a task
complete:

1. **What state changed?** (one or more fields/rows/files)
2. **Where does that state live?** (DB row+col / file path / queue topic /
   metric / external system record)
3. **How do I read it back independently?** (SQL / `Storage::get_cf` /
   `cat file` / API call / `ldb dump` / `inspect_corpus_store`)
4. **What is the exact expected value?** (literal / range / schema / count
   delta)

If you cannot answer all four, you do not understand the feature well enough
to call it done. **Ask the operator or read the relevant TECH-*.md spec
section before writing code.**

---

## 2. Trigger → Process → Outcome (the only framing for FSV)

Every feature has the shape:

```
[Trigger Event]   ──►   [Process X]   ──►   [Outcome Y]
   observable           measurable        verifiable at the SoT
```

Before implementing, write down:

- **Trigger.** What initiates the work? HTTP call / cron tick / shift-log
  append / MCP tool invocation / file system event / Edit hook.
- **Process.** What code path executes? Which functions in which files?
  Which RocksDB CF or which file path gets touched?
- **Outcome.** What is the *physical* end-state? Which CF row exists with
  what value? Which file contains what bytes? Which metric ticked up by how
  much?

If the trigger is not observable (no log/metric/event), or the outcome is
not verifiable at a SoT (no row/file/counter to read), the task is broken
*at the design level* — fix the design before writing the implementation.

---

## 3. Hard rules (non-negotiable)

1. **NO MOCKS in tests that gate task completion.** Use real RocksDB temp
   roots, real shift-log files, real `MeJepaCompiler` instances, real
   `BatchSampler`, real Docker oracle when the feature touches the corpus.
   The only legitimate fakes are deterministic *inputs* (synthetic patches,
   synthetic test outcomes) — never deterministic *replacements* for
   subsystems the feature actually exercises in production.

2. **NO BACKWARDS COMPATIBILITY.** Pre-production tree. If your change makes
   an old data file unreadable, delete the file. If it makes an old
   `serde::Deserialize` impl panic, the panic is the correct behavior. Do
   not write shims, version probes, or fallback branches. Fail fast with a
   structured error so the next agent can diagnose.

3. **NO WORKAROUNDS.** If a build fails, find the root cause and fix the
   underlying issue. If a test fails, reproduce by hand against the SoT and
   fix the bug in the code, not the test. Never weaken an assertion to make
   green; never `#[ignore]` a failing test to ship. If the operator's
   priorities force a postpone, write the blocker to `./memory/blockers/`
   with `severity: high` and surface it in the task report.

4. **TESTS MUST FAIL WHEN THE FEATURE IS BROKEN.** If you can comment out
   the core implementation and the test still passes, the test is testing
   the wrong thing — typically a hardcoded constant or a side-effect from
   setup. Mutation-test mentally: would breaking the function being tested
   produce a different test outcome? If no, rewrite the test.

5. **FAIL CLOSED ON UNKNOWNS.** Missing artifact, missing CF, missing
   environment variable, missing model file → error with a structured
   `SCREAMING_SNAKE_CASE` code (`MEJEPA_D_DRIVE_UNAVAILABLE`,
   `MEJEPA_FEEDBACK_PREDICTION_NOT_FOUND`, etc.) and exit non-zero. Never
   silently fall back to a default value, a `None`, an empty Vec, or a
   placeholder string — those hide bugs.

6. **ASSUME THE TASK IS WRONG.** Audit every claim in the `## Inputs`,
   `## Outputs`, and code-path references against the live tree before
   following them blindly. File paths get renamed; CFs change strings;
   APIs evolve. If the task says "edit `crates/X/src/Y.rs:42`" and `Y.rs`
   has 100 lines now, the line number is wrong — find the right anchor or
   ask. Treat the task as a starting hypothesis, not gospel.

7. **OCCAM'S RAZOR.** Among approaches that satisfy the DoD, prefer the one
   that adds the fewest new abstractions, files, types, and code paths.
   Three similar lines is better than a premature trait. If a feature can
   be one new function in an existing module, do not introduce a new
   module. Justify any new crate, new trait, new error enum, or new file
   in the session report.

---

## 4. Synthetic data: properties of good test inputs

Every FSV harness must drive the implementation with synthetic data that
has the following properties:

| Property | Why |
|---|---|
| **Deterministic seed** | Same input → same expected output every run |
| **Distinguishable** | Use values like `synthetic_test_user_2026_05_11` so you can find + clean later |
| **Representative** | Shape/distribution mirrors real data — never half-size or single-record |
| **Boundary-rich** | Include the §5 minimum boundary set |
| **Cleanup-tagged** | Deletable via a single query or `tempfile::TempDir` drop |

Principle: *if `2+2=4`, then `POST /add {a:2,b:2}` must return `4` AND
insert `calculations.result=4`.* The synthetic data gives you the expected
value; FSV verifies it landed at the SoT.

---

## 5. Mandatory boundary case set (≥ 3 from this list, more is better)

Pick the categories that apply to the feature:

1. **Empty** — `""`, `[]`, `{}`, `null`, missing field. Must either reject
   with a structured error OR persist a known-good empty value verifiable
   at the SoT.
2. **Single item** — off-by-one bugs surface here.
3. **Maximum allowed** — documented upper bound exactly.
4. **Maximum + 1** — must reject with a structured error code, not panic
   and not silently truncate.
5. **Wrong type / malformed** — invalid JSON, invalid UTF-8, NaN floats.
6. **Duplicate / replay** — same `idempotency_key` twice; same shift_id
   reprocessed; same `prediction_id` recorded twice.
7. **Out-of-order** — event B before event A (e.g., `TestOutcomeEvent`
   arriving before the matching `RealityPrediction`).
8. **Concurrent** — two threads or two MCP sessions writing the same CF
   simultaneously.
9. **Authorization variants** — owner, non-owner, missing token, expired
   token (where applicable).
10. **Time edges** — clock skew between writer and reader; daylight saving;
    leap second; negative `ts_millis`.

Each boundary case MUST produce an FSV evidence line — the state *before*,
the action, the state *after*, and the assertion (PASS/FAIL with
expected-vs-actual). One JSON object per case in the evidence file.

---

## 6. Required FSV evidence artifacts (a task is not done without these)

For every task, produce all of the following:

### 6.1 Build evidence

```
$ cargo build --release -p <touched-crates>
   Finished `release` profile [optimized] target(s) in <duration>
```

Capture the duration; it doubles as a perf baseline.

### 6.2 Test evidence

```
$ cargo test -p <touched-crates>
test result: ok. N passed; 0 failed; 0 ignored
```

Zero ignored. Zero failed. Number of `passed` documented in the task report.

### 6.3 Clippy evidence

```
$ cargo clippy -p <touched-crates> --no-deps --tests --examples -- -D warnings
    Finished `dev` profile in <duration>
```

`-D warnings` is non-optional. If clippy is unhappy, fix it (do not
`#[allow]` it without explaining why in the session report).

### 6.4 Source-of-Truth readback (the heart of FSV)

A JSON file at `/mnt/d/contextgraph/fsv/<task-domain>-fsv/<artifact>.json`
written by an FSV harness binary or example. The JSON includes:

```json
{
  "fsv_root": "/mnt/d/contextgraph/fsv/<task-domain>-fsv",
  "task_id": "TASK-EXAMPLE-NNN",
  "started_at_unix_ms": <int>,
  "build_release_sha": "<git HEAD short sha>",
  "happy_path": [
    {
      "case": "happy_path",
      "sot": "<CF name or file path>",
      "before": <value>,
      "trigger": "<exact command run>",
      "after": <value>,
      "expected": <value>,
      "actual": <value>,
      "pass": true,
      "evidence_path": "<absolute path to physical artifact>"
    }
  ],
  "boundary_cases": [
    { "case": "<name>", "detail": "...", "before": <v>, "after": <v>,
      "expected": <v>, "actual": <v>, "pass": true }
  ],
  "all_passed": true
}
```

### 6.5 Physical artifact verification

If the feature writes to RocksDB: list the SST files on disk and confirm
non-zero size.

```
$ find /mnt/d/contextgraph/fsv/<task>-fsv/rocks-* -name '*.sst' -printf '%p %s bytes\n'
```

If the feature writes to a file: `wc -c` and `sha256sum` the file.

If the feature emits a metric: `curl localhost:3101/metrics | grep <metric_name>`
showing the counter incremented.

### 6.6 Independent re-open verification (RocksDB only)

If you wrote to RocksDB, close the handle and reopen it in a *separate
process step* (a second `cargo run` invocation, or a separate test
function). Verify the same row exists. This catches the class of bugs
where data is in the MemTable but never flushed to the LSM tree.

### 6.7 Task report in `./memory/reports/`

`session-YYYY-MM-DD--task-<id>-<slug>.md` with:

- Outcome (complete / partial / blocked)
- Files modified/created in the repo
- Files written to `./memory/`
- Decisions made (with `./memory/decisions/` filename if a decision is
  load-bearing for future tasks)
- Discoveries (with `./memory/discoveries/` filename if surprising)
- Failures + lessons
- Successes
- Open blockers (with `./memory/blockers/` filename + severity)
- Test/build status with command output
- "Resume Here Next Session" with file paths

---

## 7. Audit-the-task checklist (run before writing any code)

For every task you pick up, audit the `## Inputs` and `## Outputs` sections
against the live tree:

```bash
# Does every file path in `## Inputs` exist?
$ for f in <listed paths>; do test -f "$f" && echo OK: $f || echo MISSING: $f; done

# Are the referenced symbols still where the task claims they are?
$ rg "pub (struct|enum|fn) <claimed-symbol>" crates/ --type rust

# Has the relevant CF list changed since the task was written?
$ rg "CF_MEJEPA_<claimed-name>" crates/context-graph-mejepa-cf/src/lib.rs

# Are the spec-refs current (file exists, section number still in the file)?
$ rg "REQ-<DOMAIN>-NN" docs/futurebuild/specs/functional/

# Does `git log` show recent commits touching the area? Read the last 5.
$ git log --oneline -5 -- crates/<touched-crate>/
```

If anything is missing or moved, **the task is wrong** — write the audit
findings into the task report and either update the task or escalate.
Do not write code against a stale path.

---

## 8. Web research and competing approaches

When a task involves an unfamiliar algorithm, a new dependency, or a design
choice the spec does not pin down:

1. **Do web research first.** Search for the canonical reference (RFC,
   paper, official docs) and read it. Cache the URL + relevant snippet in
   the session report's "Web Research Applied" section.
2. **List 2–3 competing approaches** with their pros and cons. Pick the
   Occam's-razor option (§3 rule 7) and justify why.
3. **Sketch the alternatives' migration path** in the task report — future
   you may need to switch.

For ML/embedding/RocksDB topics specifically: read the upstream `ruvnet`
implementation in `/home/cabdru/leapablememory/docs2/` (read-only) for
inspiration. Per `feedback_upstream_reference_only.md`: NEVER vendor,
path-dep, or npm-depend on those upstreams. Reimplement clean-room.

---

## 9. Occam's razor evaluation template

In every session report's "Decisions Made" section, justify the design
choice against simpler alternatives:

```markdown
## Decisions Made
- **Approach chosen**: <one-line description>
- **Considered alternatives**:
  1. <simpler approach> — rejected because <specific reason backed by code
     reference or measurement>
  2. <fancier approach> — rejected because <YAGNI / no current consumer>
- **Why this one wins**: <single sentence — usually "fewest new abstractions"
  or "matches existing pattern in <file>:<line>"
- **What changes the decision**: <future event that would justify revisiting>
```

If the spec already locked the choice, cite the requirement ID instead.

---

## 10. Failure modes that mean "task not done" even with green tests

Watch for these specifically — they're the ones that creep past
inattentive reviewers:

- **Constant returned from a function the test checks.** Test passes
  because `compute_score()` always returns `0.42`. Look at the function
  body, not just the test.
- **Test that never exercises the code path.** Conditional behind a
  feature flag that's off in tests. Run with the flag on.
- **CF row written but never flushed.** Add `db.flush_cf(cf)` before
  readback; reopen the DB in a separate handle.
- **Sync write returning Ok() but disk write never happens.** RocksDB
  has `WriteOptions::set_sync(true)` — use it for FSV writes.
- **Test that uses a different RocksDB instance than production.** Use the
  actual `open_hygiene_rocksdb` (or equivalent production opener) in the
  FSV harness, not a hand-rolled minimal opener.
- **Schema-loose JSON deserialization swallowing field renames.** Set
  `#[serde(deny_unknown_fields)]` on every persisted struct so a typo in
  the writer side fails the readback side.
- **Optional field handling that papers over a real missing value.**
  `Option<T>::unwrap_or_default()` is almost always a bug in this codebase.
- **Async tests with no await on the producer side.** If you spawn a task
  to write, you must join the task before reading.

---

## 11. Session-report contract (the last action of every task)

The very last action before declaring a task complete is writing
`./memory/reports/session-YYYY-MM-DD--task-<id>-<slug>.md` following the
SAMP template at `/home/cabdru/leapablememory/docs2/SOLO-AGENT-MEMORY-PROTOCOL.md`
§3.5 ("Phase 5: REPORT").

The report must include the absolute path of every FSV artifact you wrote,
the command line that reproduced it, the SHA of the post-build binary
(`shasum target/release/...`), and the `cargo test` / `cargo clippy`
output lines. The next agent must be able to reproduce your verification
without reading any code you wrote.

---

## 12. What the FSV evidence file *cannot* skip

- **`all_passed: true`** at the top level. If any case failed, this flag
  is `false` and the task is not done — period.
- **`cf_counts_before` / `cf_counts_after`** for any CF the task writes
  to. A silent put-to-wrong-CF would still grow the wrong CF by 1 — the
  count delta is the only signal that catches it.
- **`readback_equal: true`** per happy-path case. The harness MUST do a
  field-by-field equality check (e.g., `PartialEq` derive on the struct),
  not just a "did the row exist" check.
- **Boundary case detail strings** that name the exact invariant tested
  (e.g., `"100_000-entry bucket validates; 100_001 entries rejected"`),
  not vague labels (`"empty case"`).

Anything less is theater.

---

## Cross-references

- `/home/cabdru/leapablememory/docs2/SOLO-AGENT-MEMORY-PROTOCOL.md` — SAMP
  lifecycle (READ → ORIENT → WORK → WRITE → REPORT)
- `/home/cabdru/contextgraph/docs2/hardeningprompt.md` — full 5-axis
  hardening guide (this protocol is the verification subset)
- `/home/cabdru/contextgraph/docs2/flagprompt.md` — issue-flagging
  protocol for findings you don't fix in the same turn
- `CLAUDE.md` §11 — operator-role contract; this protocol is binding on
  every `/loop` turn
- `docs/futurebuild/13_TASK_INDEX.md` — the task DAG that this protocol
  applies to
