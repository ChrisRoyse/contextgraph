---
namespace: reports
agent_id: codex-java-hardening
timestamp: 2026-05-09T03:45:00Z
status: completed
tags: mejepa,java,ast-parser,dynamicjepa,fail-closed,fsv
---

# Java ME-JEPA AST Parser Harness Hardening

## Scope

Java only. Other language changes already present in the workspace were treated
as concurrent work and not owned by this session.

## Root Cause

The ME-JEPA Java AST harness did not encode Java ecosystem test signals:
standard Maven Surefire/Failsafe class-name patterns, `src/test/java` /
`src/it/java`, and JUnit Jupiter test annotations. The semantic-index path was
narrower than the live action path, and the live Java action compiler path could
previously downgrade missing `javac` to parser-only evidence.

## Changes

- `crates/context-graph-cli/src/bin/reality-loop.rs`
  - Added Java test recognition for Surefire/Failsafe path patterns and JUnit
    Jupiter annotations in live action AST classification.
  - Changed missing Java compiler semantics from parser-only fallback to
    `RUST_REALITY_DYNAMICJEPA_JAVA_TOOLCHAIN_MISSING` with a snapshot
    source-of-truth path.
  - Added Java FSV tests for path/annotation classification and missing-`javac`
    snapshot readback.
- `crates/context-graph-cli/src/commands/dynamicjepa.rs`
  - Mirrored the Java path/annotation rules in semantic-index extraction.
  - Added a semantic-index FSV test that writes real Java sources and
    `java-semantic-fsv.json`, then re-reads the evidence.
- `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md`
  - Documented the Java parser/action hardening and physical source of truth.
- `memory/context/mejepa-implementation-initiative.md`
  - Added Java AST parser harness hardening to the Phase 0 implementation state.
- `memory/discoveries/2026-05-09--java-mejepa-ast-test-detection-and-toolchain.md`
  - Recorded the discovery, root cause, references, and verification shape.

## Verification

- `cargo fmt --all` passed.
- `cargo test -p context-graph-cli java -- --nocapture` passed:
  - Java semantic-index Surefire fixture classified `adds` as `semantic_kind:
    test` and recorded `assertEquals`.
  - Java semantic-index qualified `@ParameterizedTest` fixture classified
    `computes` as `semantic_kind: test` and recorded `verifyReality`.
  - Java parse-error fixture failed closed with `VERIFICATION_FAILED`.
  - Java live action fixture failed closed on this host because `javac` is not
    installed; error code was
    `RUST_REALITY_DYNAMICJEPA_JAVA_TOOLCHAIN_MISSING`.
- Manual CLI FSV:
  - Source of truth:
    `/tmp/contextgraph-java-manual-fsv-2859958/java-semantic-index.json` and
    sibling evidence directory.
  - Before: output absent, evidence dir absent, real Java repo contained
    `Calc.java` and `CalculatorTests.java`.
  - Trigger:
    `cargo run -p context-graph-cli --bin context-graph-cli -- dynamicjepa build-semantic-index --repo /tmp/contextgraph-java-manual-fsv-2859958/repo --output /tmp/contextgraph-java-manual-fsv-2859958/java-semantic-index.json --language java --json`.
  - After: command exited 1, output remained absent, evidence dir existed with
    `git_rev_parse_head.stdout` and `git_rev_parse_head.stderr`, stdout reported
    missing `javac`.
- Manual snapshot readback:
  - `/tmp/contextgraph-dj-compiler-semantic-2857229-1778296998908337513/src/main/java/com/acme/Calc.java`
    exists and contains the candidate after-source with `return x + 2;`.

## Build Note

`cargo build --release -p context-graph-cli` was attempted. The normal shared
target run was blocked by other Cargo builds. An isolated target run failed in
native CUDA dependencies due out-of-memory. A constrained retry with
`CARGO_BUILD_JOBS=1 taskset -c 0` progressed past the earlier OOM point but was
stopped after the long one-at-a-time `llama-cpp-sys-2` CUDA object build ceased
to be useful Java verification signal under concurrent workspace builds.

## Web References Used

- Maven Surefire default includes:
  `Test*.java`, `*Test.java`, `*Tests.java`, `*TestCase.java`.
- Maven Failsafe default includes:
  `IT*.java`, `*IT.java`, `*ITCase.java`.
- JUnit User Guide annotations:
  `@Test`, `@ParameterizedTest`, `@RepeatedTest`, `@TestFactory`,
  `@TestTemplate`.
- Tree-sitter docs on parse recovery and missing/error nodes informed the
  fail-closed parser stance.
