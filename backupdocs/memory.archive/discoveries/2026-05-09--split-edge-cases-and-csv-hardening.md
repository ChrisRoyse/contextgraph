---
namespace: discoveries
agent_id: agent-code-simplifier-split
timestamp: 2026-05-09T00:00:00Z
status: active
tags: split,edge-cases,csv,fsv
---

# Two missing defenses: duplicate task_ids across repos, and CSV comma-injection

## Edge case missed: duplicate task_id under different repos

`split_corpus` validates non-empty `task_id` and non-empty `repo` (split.rs:133-146) but does NOT detect the case where the same `task_id` appears under two different repos. Result: that single task lands in two different buckets, breaking the "every input task appears exactly once" invariant. The FSV's invariant 1 (split_fsv.rs:148-153) uses `HashSet`, which would silently coalesce both into one entry and pass.

Cheap fix: in the loop at split.rs:131-148, before inserting into `repo_tasks`, maintain a `BTreeSet<&str>` of seen task_ids and reject duplicates with `MEJEPA_CORPUS_INVALID_INPUT`.

## CSV format: naive split-on-comma

`write_assignments_csv` (split_fsv.rs:83-94) writes `task_id,repo,bucket` with no escaping. SWE-bench Lite task_ids are `org__repo-NNN` and repos are slug-form — no commas in practice. But:
- Adding ONE assertion at write time (`assert!(!a.task_id.contains(',') && !a.repo.contains(','))`) is free, makes the assumption explicit, and prevents silent CSV corruption if the corpus shape ever extends (e.g., synthetic mutation suffix `task,foo`).
- Alternatively, emit TSV (tabs are even less likely in any identifier) — same effort, harder to misuse downstream.

Verdict: REFINEMENT-RECOMMENDED on both. Neither is a current correctness bug; both are cheap insurance.
