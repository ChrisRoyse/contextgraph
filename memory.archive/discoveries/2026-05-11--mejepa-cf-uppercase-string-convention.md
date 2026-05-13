---
namespace: discoveries
agent_id: agent-claude-opus-4.7
created: 2026-05-11T16:22:00Z
updated: 2026-05-11T16:22:00Z
status: active
tags: mejepa-cf, rocksdb, naming-convention, column-family, flywheel
---

# `mejepa-cf` constant strings use ALL_CAPS, matching the constant ident byte-for-byte

## What I found

`crates/context-graph-mejepa-cf/src/lib.rs` has an unwritten but consistent
invariant for every existing entry:

```rust
pub const CF_MEJEPA_FISHER_SNAPSHOTS: &str = "CF_MEJEPA_FISHER_SNAPSHOTS";
pub const CF_MEJEPA_DRIFT_WINDOW:    &str = "CF_MEJEPA_DRIFT_WINDOW";
// …35 more
```

The constant identifier equals the on-disk RocksDB column-family name,
character for character. This is load-bearing: a rename of the const ident
without a matching string change would silently create an empty new CF and
orphan the old one in the daemon root, with no compile-time signal.

`docs/futurebuild/specs/technical/TECH-FLYWHEEL.md §6` illustrates the three
new FLYWHEEL CFs with lowercase strings:

```rust
pub const CF_MEJEPA_DDA_SIGNALS: &str = "mejepa_dda_signals";        // spec
```

That contradicts every existing entry in `mejepa-cf/src/lib.rs`. Mixing the
two would leave the daemon opener with 35 uppercase CFs and 3 lowercase CFs
in the same RocksDB instance — no functional break, but `gh issue list`-style
greps for "CF_MEJEPA_DDA_SIGNALS" would silently miss the on-disk name, and
operator tooling that reads `db.cf_names()` to filter for the mejepa subset
would need a regex update.

## Why It Matters

For TASK-FLYWHEEL-006 I matched the existing `mejepa-cf` convention
(`CF_MEJEPA_DDA_SIGNALS = "CF_MEJEPA_DDA_SIGNALS"`) and added a unit test
(`flywheel_cf_names_match_const_idents`) that pins the invariant in CI so a
future renamer is forced through a deliberate test edit.

The 2026-05-09 corpus-side `crates/context-graph-mejepa-corpus/src/oracle.rs`
already deviates from the convention with a private `CF_MEJEPA_MUTATION_CORPUS
= "mejepa_mutation_corpus"`. That is a pre-existing inconsistency — it does
not break anything today because the corpus crate manages its own RocksDB
instance under `/mnt/d/contextgraph/corpus/<run>/oracle-store`, not the
daemon's hygiene root. If a future task wires the corpus CF into the daemon
opener it will surface as a "missing column family" error and force a
harmonization decision then.

## Related

- See also: ./memory/reports/session-2026-05-11--task-flywheel-006-new-cfs.md
- Tags: column-family, naming-convention, mejepa-cf, rocksdb
