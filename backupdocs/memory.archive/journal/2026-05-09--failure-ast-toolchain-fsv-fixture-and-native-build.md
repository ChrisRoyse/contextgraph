---
namespace: journal
created: 2026-05-08T23:44:31-05:00
updated: 2026-05-08T23:44:31-05:00
status: active
tags: failure, ast-parser, fsv, typescript, cargo
---

# Failure: AST Toolchain FSV Fixture And Native Build

## What Happened
Two failures occurred during the all-language dependency verification:

1. The first TSX synthetic fixture declared `namespace JSX` inside a module, so `tsc` correctly failed:

```text
src/view.tsx(7,21): error TS7026: JSX element implicitly has type 'any' because no interface 'JSX.IntrinsicElements' exists.
```

The fixture was corrected by declaring the JSX namespace inside `declare global { ... }`, then the real local `tsc` check passed.

2. A repo-level `cargo check -p context-graph-cli --no-default-features --bin reality-loop` using an isolated target dir was stopped after several minutes because it was compiling native RocksDB while other unrelated CUDA/Cargo work was active. This was not a missing parser dependency; the isolated `/tmp` FSV already proved the installed language tools work.

## Why It Matters
The TSX failure confirmed the verification was real and not a pass-by-construction. The Cargo check remains a heavier repo build gate and should be rerun later when native build pressure is lower.

## Related
- Evidence: `/tmp/contextgraph-ast-toolchain-fsv/toolchain-fsv.log`
- Discovery: `memory/discoveries/2026-05-09--ast-parser-toolchains-installed-and-verified.md`
- Tags: typescript, cargo, fsv
