---
namespace: journal
agent_id: codex
created: 2026-05-09T08:02:00Z
updated: 2026-05-09T08:24:44Z
timestamp: 2026-05-09T08:02:00Z
status: active
tags: journal, success, phase0, phase1, swebench, fsv
---

# Success: Phase 0 SWE-bench Manifest + Split FSV Hardening

Completed a root-cause hardening pass over Phase 0/1 recent work.

Key outcomes:

- Verified recent git history: `15522841` shipped the InstanceAtomic split,
  Rust SWE-bench oracle bridge, and ops package refactor; `f2ebf36c` shipped
  the first official harness ops script.
- Added official-manifest metadata verification to SWE-bench preflight.
- Fixed split FSV so active assignments are exact `[240,30,30]` and repo
  atomic assignments are diagnostic-only at `[195,77,28]`.
- Fixed `mejepa_corpus_fsv` so it no longer deletes sibling evidence and no
  longer overwrites the active split assignment with repo-atomic output.
- Updated live docs and specs so Phase 0 and Phase 1 status is honest after
  commit `6f72a02e` deleted the legacy `sitrep/` tree: Phase 0 now has a
  real `mejepa-corpus` CLI with one-instance/two-category Docker FSV but
  still needs the full 300-task x 8-category batch; Phase 1 now has all 15
  deterministic typed panel-slot encoders, with neural/deep integrations
  deferred.

Validation commands passed:

- `cargo test -p context-graph-mejepa-corpus`
- `cargo test -p context-graph-mejepa-instruments`
- `cargo clippy -p context-graph-mejepa-corpus -p context-graph-mejepa-instruments --all-targets -- -D warnings`
- `python3 -m py_compile scripts/swebench_lite_ops.py scripts/swebench_lite_ops/*.py`
- `python3 scripts/swebench_lite_ops.py preflight`
- `python3 scripts/swebench_lite_ops.py image-status --require-clean`
- `cargo run -p context-graph-mejepa-corpus --example mejepa_corpus_fsv`
- `cargo run -p context-graph-mejepa-corpus --example split_fsv`
- `cargo run -p context-graph-mejepa-corpus --example oracle_fsv`
- `cargo run -p context-graph-mejepa-corpus --example swebench_oracle_fsv`
- `cargo run -p context-graph-mejepa-instruments --example instruments_fsv`
- `cargo run -p context-graph-mejepa-instruments --example phase1_extended_fsv`
- `cargo run -p context-graph-mejepa-corpus --bin mejepa-corpus -- generate --output /tmp/contextgraph-mejepa-corpus-cli-fsv --instance-id sympy__sympy-20590 --category known_good --category over_engineer --run-id-prefix mejepaCliFsv`
- `cargo run -p context-graph-mejepa-corpus --bin mejepa-corpus -- verify --corpus /tmp/contextgraph-mejepa-corpus-cli-fsv`
- `cargo run -p context-graph-mejepa-corpus --bin mejepa-corpus -- stats --corpus /tmp/contextgraph-mejepa-corpus-cli-fsv`

Expected failure checks also passed with explicit evidence and error codes:

- Empty task manifest directory:
  `SWEBENCH_TASK_MANIFEST_COUNT_MISMATCH`.
- Empty expected image set with one local SWE-bench image:
  `SWEBENCH_CACHE_NOT_CLEAN`.
- Unknown SWE-bench instance id:
  `SWEBENCH_UNKNOWN_INSTANCE_ID`.
- Stale corpus output directory rejected before mutation/oracle execution.
- Invalid Docker run-id prefix rejected before output creation.
- No-site diff mutation rejected with `MEJEPA_CORPUS_NO_MUTATION_SITE`; the
  partial index remains `complete=false`, and `verify` rejects it.
