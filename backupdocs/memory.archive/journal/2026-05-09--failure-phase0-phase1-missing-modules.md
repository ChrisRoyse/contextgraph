---
namespace: journal
created: 2026-05-09T06:39:13Z
updated: 2026-05-09T06:39:13Z
status: active
tags: mejepa, phase0, phase1, build, failure, missing-modules
---

# Failure: Phase 0/1 Build Broke on Declared Missing Modules

## What Happened
`cargo test -p context-graph-mejepa-corpus -p context-graph-mejepa-instruments`
failed with `E0583` because `crates/context-graph-mejepa-corpus/src/lib.rs`
declared `pub mod oracle;` but `src/oracle.rs` did not exist. The working
tree also declared `e_witness` and `e_static_analysis` in
`context-graph-mejepa-instruments/src/lib.rs` without corresponding source
files.

## Why It Matters
The status documents said Phase 0 and Phase 1 were partial, but the code had
started wiring the next Phase 0/1 modules into public APIs. A public module
declaration without a source file makes the shipped crates uncompilable and
prevents any FSV from running.

## Related
- `crates/context-graph-mejepa-corpus/src/lib.rs`
- `crates/context-graph-mejepa-instruments/src/lib.rs`
- Fix path: implement `oracle.rs`, `e_witness.rs`, and `e_static_analysis.rs`
  as real fail-closed primitives instead of removing the public declarations.
